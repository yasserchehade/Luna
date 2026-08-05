"use client";

import { useEffect, useRef } from "react";
import { AppIcon } from "../../../components/AppIcon";
import type { HouseholdWorkStatus, HouseholdWorkView, TodayBriefing } from "../contracts";
import type { PendingConversation } from "../useToday";
import { LunaMark } from "./PrimaryNavigation";
import { EmptyBriefing, PartialFailure } from "./TodayStates";

function statusLabel(status: HouseholdWorkStatus): string {
  return {
    needsAttention: "Needs your attention",
    awaitingApproval: "Awaiting approval",
    needsClarification: "Needs clarification",
    upcoming: "Upcoming",
    completed: "Completed",
    dismissed: "Dismissed",
  }[status];
}

function statusIcon(status: HouseholdWorkStatus) {
  if (status === "completed") return "check" as const;
  if (status === "upcoming") return "clock" as const;
  return "alert" as const;
}

export function BriefingHeader({ briefing, onOpenMenu, onOpenContext }: {
  briefing: TodayBriefing;
  onOpenMenu: () => void;
  onOpenContext: () => void;
}) {
  return (
    <header className="today-workspace-header">
      <button className="today-mobile-menu-button" type="button" aria-label="Open navigation" onClick={onOpenMenu}><AppIcon name="menu" /></button>
      <div className="today-heading">
        <span className="eyebrow">{briefing.dateLabel}</span>
        <h1>{briefing.greeting}, {briefing.member.displayName}.</h1>
      </div>
      <button className="today-context-button" type="button" onClick={onOpenContext}><AppIcon name="details" /> Work details</button>
    </header>
  );
}

function BriefingIntro({ briefing }: { briefing: TodayBriefing }) {
  const { reviewed } = briefing;
  return (
    <section className="today-briefing-intro" aria-label="Luna briefing">
      <LunaMark />
      <div>
        <p>While you were away, I reviewed <strong>{reviewed.emails} emails</strong>, <strong>{reviewed.documents} documents</strong>{reviewed.calendar ? " and your calendar" : ""}.</p>
        <p>Here is what needs your attention today.</p>
      </div>
    </section>
  );
}

function PendingConversationEntry({ pending }: { pending: PendingConversation }) {
  const entry = useRef<HTMLDivElement>(null);

  useEffect(() => {
    entry.current?.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
  }, []);

  return (
    <div ref={entry} className="conversation-entry member" data-pending="true">
      <strong>You</strong>
      <p>{pending.entry.body}</p>
    </div>
  );
}

function HouseholdConversation({ briefing, pending }: { briefing: TodayBriefing; pending: PendingConversation | null }) {
  if (briefing.conversation.length === 0 && !pending) return null;
  return (
    <section className="today-conversation" role="log" aria-label="Household conversation" aria-live="polite" aria-relevant="additions">
      {briefing.conversation.map((entry) => (
        <div className={`conversation-entry ${entry.role}`} key={entry.id} data-created-at={entry.createdAt}>
          <strong>{entry.role === "luna" ? "Luna" : "You"}</strong>
          <p>{entry.body}</p>
        </div>
      ))}
      {pending && <PendingConversationEntry pending={pending} />}
    </section>
  );
}

export function HouseholdWorkReport({
  work,
  selected,
  compact = false,
  pending,
  error,
  onRetry,
  onSelect,
  onApprove,
  onDiscuss,
  onComplete,
  onDismiss,
}: {
  work: HouseholdWorkView;
  selected: boolean;
  compact?: boolean;
  pending: boolean;
  error?: string | null;
  onRetry: () => void;
  onSelect: () => void;
  onApprove: () => void;
  onDiscuss: () => void;
  onComplete: () => void;
  onDismiss: () => void;
}) {
  return (
    <article className={`today-work-report ${selected ? "selected" : ""} ${compact ? "compact" : ""}`} data-status={work.status} aria-busy={pending || undefined}>
      <button type="button" className="today-work-heading" onClick={onSelect} aria-label={`Open ${work.title}`}>
        <span className="status-symbol"><AppIcon name={statusIcon(work.status)} /></span>
        <span className="work-title-copy"><span className="eyebrow">{statusLabel(work.status)}</span><strong>{work.title}</strong></span>
        <span className="work-timing">{work.dueLabel}</span>
      </button>
      <div className="today-work-body">
        <p>{work.summary}</p>
        {!compact && (
          <>
            <div className="work-source"><AppIcon name="source" /><span>{work.source.label}</span><span aria-hidden="true">·</span><span>{work.householdEntity}</span>{work.amountLabel && <><span aria-hidden="true">·</span><span>{work.amountLabel}</span></>}</div>
            <div className="work-recommendation"><span>Luna recommends</span><p>{work.recommendation}</p></div>
          </>
        )}
        {!compact && work.status !== "completed" && work.status !== "dismissed" && (
          <div className="today-work-actions" aria-label={`Actions for ${work.title}`}>
            {work.proposedAction && <button className="primary-action" type="button" disabled={pending} onClick={onApprove}>{pending ? "Saving…" : work.proposedAction.label}</button>}
            <button type="button" disabled={pending} onClick={onDiscuss}>Discuss</button>
            <button type="button" disabled={pending} onClick={onComplete}>Mark complete</button>
            <button className="quiet-action" type="button" disabled={pending} onClick={onDismiss}>Dismiss</button>
          </div>
        )}
        {error && selected && <div className="work-action-error" role="alert"><span>{error}</span><button type="button" onClick={onRetry}>Retry</button></div>}
      </div>
    </article>
  );
}

function BriefingSection({ title, children }: { title: string; children: React.ReactNode }) {
  return <section className="today-briefing-section"><h2>{title}</h2><div className="today-briefing-list">{children}</div></section>;
}

export function BriefingStream({
  briefing,
  selectedWorkId,
  pendingWorkId,
  actionError,
  onRetry,
  onSelect,
  onApprove,
  onDiscuss,
  onComplete,
  onDismiss,
  pendingConversation,
}: {
  briefing: TodayBriefing;
  selectedWorkId: string | null;
  pendingWorkId: string | null;
  actionError: string | null;
  onRetry: () => void;
  onSelect: (id: string) => void;
  onApprove: (work: HouseholdWorkView) => void;
  onDiscuss: (work: HouseholdWorkView) => void;
  onComplete: (id: string) => void;
  onDismiss: (id: string) => void;
  pendingConversation: PendingConversation | null;
}) {
  const visible = briefing.work.filter((work) => work.status !== "dismissed");
  const attention = visible.filter((work) => ["needsAttention", "awaitingApproval", "needsClarification"].includes(work.status));
  const upcoming = visible.filter((work) => work.status === "upcoming");
  const completed = visible.filter((work) => work.status === "completed");

  const report = (work: HouseholdWorkView, compact = false) => (
    <HouseholdWorkReport
      key={work.id}
      work={work}
      selected={selectedWorkId === work.id}
      compact={compact}
      pending={pendingWorkId === work.id}
      error={actionError}
      onRetry={onRetry}
      onSelect={() => onSelect(work.id)}
      onApprove={() => onApprove(work)}
      onDiscuss={() => onDiscuss(work)}
      onComplete={() => onComplete(work.id)}
      onDismiss={() => onDismiss(work.id)}
    />
  );

  if (visible.length === 0 && briefing.partialFailures.length === 0) {
    return (
      <div className="today-briefing-stream">
        <BriefingIntro briefing={briefing} />
        <HouseholdConversation briefing={briefing} pending={pendingConversation} />
        <EmptyBriefing />
      </div>
    );
  }

  return (
    <div className="today-briefing-stream">
      <BriefingIntro briefing={briefing} />
      <HouseholdConversation briefing={briefing} pending={pendingConversation} />
      {briefing.partialFailures.map((failure) => <PartialFailure key={failure.id} title={failure.title} message={failure.message} />)}
      {attention.length > 0 ? <BriefingSection title="Needs your attention">{attention.map((work) => report(work))}</BriefingSection> : (
        <section className="nothing-urgent"><AppIcon name="check" /><div><h2>Nothing urgent</h2><p>I do not need a decision from you right now.</p></div></section>
      )}
      {upcoming.length > 0 && <BriefingSection title="Upcoming">{upcoming.map((work) => report(work, true))}</BriefingSection>}
      {completed.length > 0 && <BriefingSection title="Completed while you were away">{completed.map((work) => report(work, work.id !== selectedWorkId))}</BriefingSection>}
    </div>
  );
}
