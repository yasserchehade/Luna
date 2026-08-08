"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  TodayServiceError,
  type AttachmentResult,
  type ConversationMessage,
  type ConversationResult,
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
  entry: ConversationMessage;
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
  const [conversationContextWorkId, setConversationContextWorkId] = useState<string | null>(null);
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
      setSelectedWorkId((current) => current && next.work.some((work) => work.id === current) ? current : null);
      setConversationContextWorkId((current) => current && next.work.some((work) => work.id === current) ? current : null);
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

  const applyMutationResult = useCallback((result: MutationResult) => {
    setBriefing(result.briefing);
    setNotice(result.confirmation);
    setActionError(null);
    setFailedMutation(null);
    if (result.work?.status === "dismissed") {
      setSelectedWorkId((current) => current === result.work?.id ? null : current);
      setConversationContextWorkId((current) => current === result.work?.id ? null : current);
      setCorrectionOpen(false);
    }
  }, []);

  const applyConversationResult = useCallback((result: ConversationResult) => {
    setBriefing(result.briefing);
    setNotice(result.lunaMessage.body);
    setActionError(null);
    setFailedMutation(null);
    setConversationContextWorkId((current) => {
      if (!current) return null;
      const work = result.briefing.work.find((candidate) => candidate.id === current);
      return work?.status === "dismissed" ? null : current;
    });
  }, []);

  const runMutation = useCallback(async (mutation: PendingMutation) => {
    setPendingMutation(mutation);
    setActionError(null);
    try {
      if (mutation.kind === "approve") {
        applyMutationResult(await service.approveAction(mutation.workId, mutation.actionId));
      } else if (mutation.kind === "dismiss") {
        applyMutationResult(await service.dismissWork(mutation.workId));
      } else if (mutation.kind === "complete") {
        applyMutationResult(await service.completeWork(mutation.workId));
      } else {
        applyMutationResult(await service.correctFact(mutation.input));
        setCorrectionOpen(false);
      }
    } catch (error) {
      setActionError(safeMessage(error));
      setFailedMutation(mutation);
    } finally {
      setPendingMutation(null);
    }
  }, [applyMutationResult, service]);

  const send = useCallback(async () => {
    if (sendingRef.current || (!draft.trim() && !attachment)) return;
    sendingRef.current = true;
    setSending(true);
    setActionError(null);
    const message = draft.trim();
    if (message) {
      setPendingConversation({
        entry: {
          id: "pending-member-message",
          role: "member",
          body: message,
          createdAt: new Date().toISOString(),
          ...(conversationContextWorkId ? { contextualWorkIds: [conversationContextWorkId] } : {}),
        },
      });
    }
    try {
      const result = await service.sendMessage({
        message: draft,
        contextualWorkIds: conversationContextWorkId ? [conversationContextWorkId] : undefined,
        attachmentId: attachment?.attachmentId,
      });
      applyConversationResult(result);
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
  }, [applyConversationResult, attachment, conversationContextWorkId, draft, service, setDraft]);

  const attach = useCallback(async (file: File) => {
    setAttachmentPending(true);
    setActionError(null);
    try {
      const result = await service.attachSource(file);
      setAttachment(result);
      setNotice(result.persisted
        ? `${result.displayName} is uploaded and ready to discuss.`
        : `${result.displayName} is ready to discuss. Nothing has been uploaded.`);
    } catch (error) {
      setActionError(safeMessage(error));
    } finally {
      setAttachmentPending(false);
    }
  }, [service]);

  const selectWork = useCallback((workId: string, openContext = false) => {
    setSelectedWorkId(workId);
    setConversationContextWorkId(workId);
    setCorrectionOpen(false);
    setActionError(null);
    if (openContext) setContextOpen(true);
  }, []);

  const clearConversationContext = useCallback(() => {
    setConversationContextWorkId(null);
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
  const conversationContextWork = briefing?.work.find((work) => work.id === conversationContextWorkId) ?? null;

  return {
    briefing,
    loading,
    loadError,
    reload: load,
    activeNavigation,
    navigate,
    selectedWork,
    conversationContextWork,
    selectWork,
    clearConversationContext,
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
