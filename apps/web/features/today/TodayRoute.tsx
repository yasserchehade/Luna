"use client";

import { useEffect, useMemo, useState } from "react";
import { createMockTodayService } from "./mockTodayService";
import type { HouseholdWorkView, TodayService } from "./contracts";
import { useToday } from "./useToday";
import { BriefingHeader, BriefingStream } from "./components/BriefingStream";
import { PersistentComposer } from "./components/PersistentComposer";
import { MobileMenu, MobileNavigation, PrimaryNavigation } from "./components/PrimaryNavigation";
import { BriefingError, BriefingSkeleton, PlaceholderDestination } from "./components/TodayStates";
import { WorkingContextPanel } from "./components/WorkingContextPanel";

type LayoutMode = "mobile" | "tablet" | "desktop";

function layoutForWidth(width: number): LayoutMode {
  if (width < 720) return "mobile";
  if (width < 1120) return "tablet";
  return "desktop";
}

export function TodayRoute({ service }: { service?: TodayService }) {
  const todayService = useMemo(() => service ?? createMockTodayService(), [service]);
  const today = useToday(todayService);
  const [layout, setLayout] = useState<LayoutMode>("desktop");
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  useEffect(() => {
    const update = () => setLayout(layoutForWidth(window.innerWidth));
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  const pendingWorkId = today.pendingMutation
    ? "workId" in today.pendingMutation
      ? today.pendingMutation.workId
      : today.pendingMutation.input.workId
    : null;

  const member = today.briefing?.member ?? { householdName: "Luna household", initials: "LH" };
  const approve = (work: HouseholdWorkView) => {
    if (work.proposedAction) void today.approve(work.id, work.proposedAction.id);
  };

  return (
    <div className="today-shell" data-layout={layout}>
      <PrimaryNavigation active={today.activeNavigation} household={member.householdName} initials={member.initials} onNavigate={today.navigate} />
      <main className="today-main" id="main-content">
        {today.briefing && <BriefingHeader briefing={today.briefing} onOpenMenu={() => setMobileMenuOpen(true)} onOpenContext={() => today.setContextOpen(true)} />}
        {!today.briefing && <header className="today-workspace-header loading"><div className="today-heading"><span className="eyebrow">Today</span><h1>Luna is getting ready.</h1></div></header>}

        <div className="today-scroll-region">
          <div className="today-announcements" aria-live="polite" aria-atomic="true">
            {today.notice && <div className="today-notice" role="status">{today.notice}</div>}
            {today.actionError && (!today.failedMutation || !today.selectedWork) && (
              <div className="today-inline-error" role="alert">
                <span>{today.actionError}</span>
                {(today.failedSend || today.failedMutation) && (
                  <button type="button" onClick={() => today.failedMutation ? void today.retryMutation() : void today.send()}>Retry</button>
                )}
              </div>
            )}
          </div>

          {today.activeNavigation !== "Today" ? <PlaceholderDestination destination={today.activeNavigation} /> : today.loading ? <BriefingSkeleton /> : today.loadError ? (
            <BriefingError message={today.loadError} onRetry={() => void today.reload()} />
          ) : today.briefing ? (
            <BriefingStream
              briefing={today.briefing}
              selectedWorkId={today.selectedWork?.id ?? null}
              pendingWorkId={pendingWorkId}
              actionError={today.failedMutation ? today.actionError : null}
              onRetry={() => void today.retryMutation()}
              onSelect={(id) => today.selectWork(id, layout !== "desktop")}
              onApprove={approve}
              onDiscuss={(work) => today.discuss(work.id, work.title)}
              onComplete={(id) => void today.complete(id)}
              onDismiss={(id) => void today.dismiss(id)}
              pendingConversation={today.pendingConversation}
            />
          ) : null}
        </div>

        <PersistentComposer
          contextualWork={today.conversationContextWork}
          draft={today.draft}
          attachment={today.attachment}
          sending={today.sending}
          attachmentPending={today.attachmentPending}
          onDraftChange={today.setDraft}
          onClearContext={today.clearConversationContext}
          onAttach={(file) => void today.attach(file)}
          onClearAttachment={today.clearAttachment}
          onSend={() => void today.send()}
        />
      </main>

      {layout === "desktop" && (
        <WorkingContextPanel
          work={today.selectedWork}
          correctionOpen={today.correctionOpen}
          pending={pendingWorkId === today.selectedWork?.id}
          onClose={() => today.setContextOpen(false)}
          onOpenCorrection={() => today.setCorrectionOpen(true)}
          onCancelCorrection={() => today.setCorrectionOpen(false)}
          onCorrect={(input) => void today.correct(input)}
        />
      )}

      {(layout === "tablet" || layout === "mobile") && today.contextOpen && (
        <div className="today-drawer-backdrop" onMouseDown={() => today.setContextOpen(false)}>
          <div onMouseDown={(event) => event.stopPropagation()}>
            <WorkingContextPanel
              work={today.selectedWork}
              drawer
              correctionOpen={today.correctionOpen}
              pending={pendingWorkId === today.selectedWork?.id}
              onClose={() => today.setContextOpen(false)}
              onOpenCorrection={() => today.setCorrectionOpen(true)}
              onCancelCorrection={() => today.setCorrectionOpen(false)}
              onCorrect={(input) => void today.correct(input)}
            />
          </div>
        </div>
      )}

      {layout === "mobile" && <MobileNavigation active={today.activeNavigation} onNavigate={today.navigate} />}
      <MobileMenu active={today.activeNavigation} open={mobileMenuOpen} onClose={() => setMobileMenuOpen(false)} onNavigate={today.navigate} />
    </div>
  );
}
