"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  TodayServiceError,
  type AttachmentResult,
  type ConversationEntry,
  type FactCorrectionInput,
  type MutationResult,
  type TodayBriefing,
  type TodayNavigationKey,
  type TodayService,
} from "./contracts";

type PendingMutation =
  | { kind: "approve"; workId: string; actionId: string }
  | { kind: "dismiss"; workId: string }
  | { kind: "complete"; workId: string }
  | { kind: "correct"; input: FactCorrectionInput };

const DRAFT_KEY = "luna.today.draft";

export type PendingConversation = {
  workId: string | null;
  entry: ConversationEntry;
};

function safeMessage(error: unknown): string {
  if (error instanceof TodayServiceError) return error.message;
  return "I could not save that change. Your household work was not changed.";
}

export function useToday(service: TodayService) {
  const [briefing, setBriefing] = useState<TodayBriefing | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [activeNavigation, setActiveNavigation] = useState<TodayNavigationKey>("Today");
  const [selectedWorkId, setSelectedWorkId] = useState<string | null>(null);
  const [contextOpen, setContextOpen] = useState(false);
  const [correctionOpen, setCorrectionOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pendingMutation, setPendingMutation] = useState<PendingMutation | null>(null);
  const [failedMutation, setFailedMutation] = useState<PendingMutation | null>(null);
  const [attachment, setAttachment] = useState<AttachmentResult | null>(null);
  const [attachmentPending, setAttachmentPending] = useState(false);
  const [draft, setDraftState] = useState("");
  const [sending, setSending] = useState(false);
  const [failedSend, setFailedSend] = useState(false);
  const [pendingConversation, setPendingConversation] = useState<PendingConversation | null>(null);
  const sendingRef = useRef(false);

  const setDraft = useCallback((value: string) => {
    setDraftState(value);
    try {
      window.localStorage.setItem(DRAFT_KEY, value);
    } catch {
      // Draft persistence is a convenience; the composer remains usable without it.
    }
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const next = await service.getBriefing();
      setBriefing(next);
      setSelectedWorkId((current) => current ?? next.work.find((work) => work.status !== "dismissed")?.id ?? null);
    } catch (error) {
      setLoadError(safeMessage(error));
    } finally {
      setLoading(false);
    }
  }, [service]);

  useEffect(() => {
    try {
      setDraftState(window.localStorage.getItem(DRAFT_KEY) ?? "");
    } catch {
      setDraftState("");
    }
    void load();
  }, [load]);

  const applyResult = useCallback((result: MutationResult) => {
    setBriefing(result.briefing);
    setNotice(result.confirmation);
    setActionError(null);
    setFailedMutation(null);
    if (result.work?.status === "dismissed") {
      setSelectedWorkId(result.briefing.work.find((work) => work.status !== "dismissed")?.id ?? null);
      setCorrectionOpen(false);
    } else if (result.work) {
      setSelectedWorkId(result.work.id);
    }
  }, []);

  const runMutation = useCallback(async (mutation: PendingMutation) => {
    setPendingMutation(mutation);
    setActionError(null);
    try {
      if (mutation.kind === "approve") {
        applyResult(await service.approveAction(mutation.workId, mutation.actionId));
      } else if (mutation.kind === "dismiss") {
        applyResult(await service.dismissWork(mutation.workId));
      } else if (mutation.kind === "complete") {
        applyResult(await service.completeWork(mutation.workId));
      } else {
        applyResult(await service.correctFact(mutation.input));
        setCorrectionOpen(false);
      }
    } catch (error) {
      setActionError(safeMessage(error));
      setFailedMutation(mutation);
    } finally {
      setPendingMutation(null);
    }
  }, [applyResult, service]);

  const send = useCallback(async () => {
    if (sendingRef.current || (!draft.trim() && !attachment)) return;
    sendingRef.current = true;
    setSending(true);
    setActionError(null);
    const message = draft.trim();
    if (message) {
      setPendingConversation({
        workId: selectedWorkId,
        entry: { id: "pending-member-message", speaker: "member", message },
      });
    }
    try {
      const result = await service.sendMessage({
        message: draft,
        workId: selectedWorkId ?? undefined,
        attachmentId: attachment?.attachmentId,
      });
      applyResult(result);
      setDraft("");
      setAttachment(null);
      setFailedSend(false);
    } catch (error) {
      setActionError(safeMessage(error));
      setFailedSend(true);
    } finally {
      setPendingConversation(null);
      sendingRef.current = false;
      setSending(false);
    }
  }, [applyResult, attachment, draft, selectedWorkId, service, setDraft]);

  const attach = useCallback(async (file: File) => {
    setAttachmentPending(true);
    setActionError(null);
    try {
      const result = await service.attachSource(file);
      setAttachment(result);
      setNotice(`${result.displayName} is ready to discuss. Nothing has been uploaded.`);
    } catch (error) {
      setActionError(safeMessage(error));
    } finally {
      setAttachmentPending(false);
    }
  }, [service]);

  const selectWork = useCallback((workId: string, openContext = false) => {
    setSelectedWorkId(workId);
    setCorrectionOpen(false);
    setActionError(null);
    if (openContext) setContextOpen(true);
  }, []);

  const clearWorkContext = useCallback(() => {
    setSelectedWorkId(null);
    setCorrectionOpen(false);
    setContextOpen(false);
  }, []);

  const discuss = useCallback((workId: string, title: string) => {
    selectWork(workId);
    setDraft(`Let's discuss ${title.toLowerCase()}. `);
  }, [selectWork, setDraft]);

  const navigate = useCallback((destination: TodayNavigationKey) => {
    setActiveNavigation(destination);
    setContextOpen(false);
    setNotice(null);
  }, []);

  const selectedWork = briefing?.work.find((work) => work.id === selectedWorkId) ?? null;

  return {
    briefing,
    loading,
    loadError,
    reload: load,
    activeNavigation,
    navigate,
    selectedWork,
    selectWork,
    clearWorkContext,
    contextOpen,
    setContextOpen,
    correctionOpen,
    setCorrectionOpen,
    notice,
    actionError,
    pendingMutation,
    failedMutation,
    retryMutation: () => failedMutation && runMutation(failedMutation),
    approve: (workId: string, actionId: string) => runMutation({ kind: "approve", workId, actionId }),
    dismiss: (workId: string) => runMutation({ kind: "dismiss", workId }),
    complete: (workId: string) => runMutation({ kind: "complete", workId }),
    correct: (input: FactCorrectionInput) => runMutation({ kind: "correct", input }),
    discuss,
    draft,
    setDraft,
    send,
    failedSend,
    sending,
    pendingConversation,
    attachment,
    attach,
    clearAttachment: () => setAttachment(null),
    attachmentPending,
  };
}
