"use client";

import { useEffect, useReducer, useRef, useState, type ChangeEvent, type Dispatch, type FormEvent, type ReactNode } from "react";
import { AppIcon, type IconName } from "./AppIcon";
import { PrototypeSwitcher } from "./PrototypeSwitcher";
import {
  createInitialState,
  layoutModeForWidth,
  prototypeReducer,
  visibleAttention,
  type HouseholdWork,
  type NavigationKey,
  type PrototypeFixtureState,
  type PrototypeAction,
  type PrototypeVariantKey,
} from "../lib/prototypeState";

// Three variants of Luna's web-first Today experience, switchable via ?variant= on /prototype/web-first.

const navigation: Array<{ label: NavigationKey; icon: IconName }> = [
  { label: "Today", icon: "today" },
  { label: "Conversations", icon: "conversation" },
  { label: "Calendar", icon: "calendar" },
  { label: "Cabinet", icon: "cabinet" },
  { label: "Household", icon: "household" },
  { label: "History", icon: "history" },
  { label: "Settings", icon: "settings" },
];

function statusLabel(status: HouseholdWork["status"]) {
  return ({
    attention: "Needs your input",
    awaitingApproval: "Approval needed",
    upcoming: "Upcoming",
    completed: "Completed",
    dismissed: "Dismissed",
  } as const)[status];
}

function LunaMark() {
  return <span className="luna-mark" aria-hidden="true"><AppIcon name="spark" /></span>;
}

function Sidebar({ active, onNavigate }: { active: NavigationKey; onNavigate: (destination: NavigationKey) => void }) {
  return (
    <aside className="sidebar">
      <div className="brand"><LunaMark /><strong>Luna</strong></div>
      <nav aria-label="Primary navigation">
        {navigation.map((item) => (
          <button key={item.label} type="button" className={active === item.label ? "active" : ""} onClick={() => onNavigate(item.label)} aria-current={active === item.label ? "page" : undefined}>
            <AppIcon name={item.icon} /> <span>{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="household-switcher">
        <span>YC</span><div><strong>Chehade household</strong><small>All systems normal</small></div>
      </div>
    </aside>
  );
}

function MobileNavigation({ active, onNavigate }: { active: NavigationKey; onNavigate: (destination: NavigationKey) => void }) {
  return (
    <nav className="mobile-navigation" aria-label="Mobile navigation">
      {navigation.slice(0, 5).map((item) => (
        <button key={item.label} type="button" className={active === item.label ? "active" : ""} onClick={() => onNavigate(item.label)} aria-label={item.label}>
          <AppIcon name={item.icon} /><span>{item.label}</span>
        </button>
      ))}
    </nav>
  );
}

function WorkReport({ work, selected, onSelect, dispatch, compact = false }: {
  work: HouseholdWork;
  selected: boolean;
  onSelect: () => void;
  dispatch: Dispatch<PrototypeAction>;
  compact?: boolean;
}) {
  return (
    <article className={`work-report ${selected ? "selected" : ""} ${compact ? "compact" : ""}`} data-status={work.status}>
      <button type="button" className="work-report-heading" onClick={onSelect} aria-label={`Open ${work.title}`}>
        <span className="status-dot"><AppIcon name={work.status === "completed" ? "check" : work.status === "upcoming" ? "clock" : "alert"} /></span>
        <span className="work-copy"><span className="eyebrow">{statusLabel(work.status)}</span><strong>{work.title}</strong></span>
        <span className="work-due">{work.due}</span>
      </button>
      <div className="work-report-body">
        <p>{work.summary}</p>
        {!compact && <div className="source-line"><AppIcon name="source" /><span>{work.source}</span><span>·</span><span>{work.entity}</span></div>}
        {!compact && <div className="recommendation"><span>Luna recommends</span><p>{work.recommendation}</p></div>}
        {work.status !== "completed" && work.status !== "dismissed" && (
          <div className="work-actions">
            {work.status === "awaitingApproval" && <button className="primary-action" type="button" onClick={() => dispatch({ type: "approve", workId: work.id })}>Approve reminder</button>}
            <button type="button" onClick={() => dispatch({ type: "discuss", workId: work.id })}>Discuss</button>
            <button type="button" onClick={() => dispatch({ type: "complete", workId: work.id })}>Mark complete</button>
            <button className="quiet-action" type="button" onClick={() => dispatch({ type: "dismiss", workId: work.id })}>Dismiss</button>
          </div>
        )}
      </div>
    </article>
  );
}

function ContextPanel({ work, correctionOpen, dispatch, drawer = false }: {
  work: HouseholdWork | null;
  correctionOpen: boolean;
  dispatch: Dispatch<PrototypeAction>;
  drawer?: boolean;
}) {
  const [correction, setCorrection] = useState(work?.summary ?? "");
  useEffect(() => setCorrection(work?.summary ?? ""), [work?.id, work?.summary]);

  return (
    <aside className={`context-panel ${drawer ? "context-drawer" : ""}`} aria-label="Working context">
      <header>
        <div><span>Working context</span><strong>{work?.title ?? "Nothing selected"}</strong></div>
        {drawer && <button type="button" aria-label="Close work details" onClick={() => dispatch({ type: "toggleContext", open: false })}><AppIcon name="close" /></button>}
      </header>
      {work ? (
        <div className="context-content">
          <section><span className="context-label">Currently working on</span><p>{work.activity}</p></section>
          <section><span className="context-label">Relevant source</span><div className="context-source"><AppIcon name="source" /><div><strong>{work.source}</strong><small>{work.sourceDetail}</small></div></div></section>
          <section><span className="context-label">Household</span><div className="entity-pill"><AppIcon name="home" />{work.entity}</div></section>
          <section>
            <span className="context-label">What I understand</span>
            <dl>{work.facts.map((fact) => <div key={fact.label}><dt>{fact.label}</dt><dd>{fact.value}</dd></div>)}</dl>
          </section>
          <section><span className="context-label">What I still need</span><p>{work.needs ?? "Nothing from you right now."}</p></section>
          <section><span className="context-label">Proposed next step</span><p className="context-recommendation">{work.recommendation}</p></section>
          {correctionOpen ? (
            <form className="correction-form" onSubmit={(event) => { event.preventDefault(); dispatch({ type: "saveCorrection", workId: work.id, value: correction }); }}>
              <label htmlFor="correction">Correct Luna&apos;s understanding</label>
              <textarea id="correction" value={correction} onChange={(event) => setCorrection(event.target.value)} />
              <div><button className="primary-action" type="submit">Save correction</button><button type="button" onClick={() => dispatch({ type: "cancelCorrection" })}>Cancel</button></div>
            </form>
          ) : (
            <button className="details-button" type="button" onClick={() => dispatch({ type: "openCorrection", workId: work.id })}><AppIcon name="details" /> Correct details</button>
          )}
          <button className="evidence-link" type="button">View source and evidence</button>
        </div>
      ) : <div className="context-empty"><AppIcon name="details" /><p>Select Household Work to see the context Luna is using.</p></div>}
    </aside>
  );
}

function Composer({ state, dispatch }: { state: ReturnType<typeof createInitialState>; dispatch: Dispatch<PrototypeAction> }) {
  const fileInput = useRef<HTMLInputElement>(null);
  const send = (event: FormEvent) => { event.preventDefault(); dispatch({ type: "send" }); };
  const attach = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) dispatch({ type: "attach", filename: file.name });
  };
  return (
    <form className="composer" onSubmit={send} aria-label="Message Luna">
      {(state.selectedWorkId || state.attachmentName) && <div className="composer-context" aria-label="Current conversation context">
        {state.selectedWorkId && <span><AppIcon name="spark" />{state.works.find((work) => work.id === state.selectedWorkId)?.title}</span>}
        {state.attachmentName && <span><AppIcon name="paperclip" />{state.attachmentName}</span>}
      </div>}
      <div className="composer-row">
        <input ref={fileInput} className="visually-hidden" type="file" accept=".pdf,.png,.jpg,.jpeg" onChange={attach} aria-label="Attach a household document" />
        <button type="button" className="icon-button" onClick={() => fileInput.current?.click()} aria-label="Attach a household document"><AppIcon name="paperclip" /></button>
        <textarea aria-label="Message" rows={1} value={state.composer} onChange={(event) => dispatch({ type: "setComposer", value: event.target.value })} placeholder="What would you like me to take care of?" />
        <button type="submit" className="send-button" aria-label="Send message"><AppIcon name="send" /></button>
      </div>
      <small>Mock prototype · no information leaves this browser</small>
    </form>
  );
}

function BriefingHeader({ onOpenContext }: { onOpenContext: () => void }) {
  return (
    <header className="workspace-header">
      <div className="mobile-brand"><LunaMark /><strong>Luna</strong></div>
      <div><span className="eyebrow">Tuesday, 4 August</span><h1>Good evening, Yasser.</h1></div>
      <button className="mobile-details" type="button" onClick={onOpenContext}><AppIcon name="details" /> Work details</button>
    </header>
  );
}

function BriefingIntro() {
  return (
    <div className="briefing-intro">
      <LunaMark />
      <div><p>While you were away I reviewed <strong>24 new emails</strong>, <strong>two documents</strong> and your calendar.</p><p>Here is what needs your attention today.</p></div>
    </div>
  );
}

function VariantA({ works, selectedId, dispatch }: VariantProps) {
  const attention = visibleAttention(works);
  const upcoming = works.filter((work) => work.status === "upcoming");
  const completed = works.filter((work) => work.status === "completed");
  return (
    <div className="variant-content variant-a">
      <BriefingIntro />
      <BriefingSection title="Needs your attention" count={attention.length}>{attention.map((work) => <WorkReport key={work.id} work={work} selected={selectedId === work.id} onSelect={() => dispatch({ type: "selectWork", workId: work.id })} dispatch={dispatch} />)}</BriefingSection>
      <BriefingSection title="Upcoming" count={upcoming.length}>{upcoming.map((work) => <WorkReport key={work.id} work={work} selected={selectedId === work.id} onSelect={() => dispatch({ type: "selectWork", workId: work.id })} dispatch={dispatch} compact />)}</BriefingSection>
      <BriefingSection title="Completed while you were away" count={completed.length}>{completed.map((work) => <WorkReport key={work.id} work={work} selected={selectedId === work.id} onSelect={() => dispatch({ type: "selectWork", workId: work.id })} dispatch={dispatch} compact />)}</BriefingSection>
    </div>
  );
}

function VariantB({ works, selectedId, dispatch }: VariantProps) {
  const open = works.filter((work) => work.status !== "completed" && work.status !== "dismissed");
  return (
    <div className="variant-content variant-b">
      <div className="desk-summary"><span>Today at a glance</span><strong>2 decisions</strong><strong>1 upcoming</strong><strong>1 completed</strong></div>
      <BriefingIntro />
      <div className="desk-list">
        <div className="desk-rail"><span>Now</span><i /><span>Next</span><i /><span>Done</span></div>
        <div>{open.map((work) => <WorkReport key={work.id} work={work} selected={selectedId === work.id} onSelect={() => dispatch({ type: "selectWork", workId: work.id })} dispatch={dispatch} />)}</div>
      </div>
    </div>
  );
}

function VariantC({ works, selectedId, dispatch }: VariantProps) {
  const selected = works.find((work) => work.id === selectedId) ?? works[0];
  return (
    <div className="variant-content variant-c">
      <div className="conversation-turn luna-turn"><LunaMark /><div><span>Luna · just now</span><p>Good evening, Yasser. I kept things moving while you were away.</p><p>I completed one follow-up, found two matters that need you, and prepared one upcoming form.</p></div></div>
      <div className="conversation-brief"><span>Most useful next step</span><WorkReport work={selected} selected onSelect={() => dispatch({ type: "selectWork", workId: selected.id })} dispatch={dispatch} /></div>
      <div className="conversation-turn luna-turn"><LunaMark /><div><p>I recommend approving the electricity reminder first. The insurance renewal deserves a short discussion because the excess increased.</p><div className="inline-work-links">{works.filter((work) => work.status === "attention").map((work) => <button type="button" key={work.id} onClick={() => dispatch({ type: "selectWork", workId: work.id })}>{work.title} <span>→</span></button>)}</div></div></div>
    </div>
  );
}

type VariantProps = { works: HouseholdWork[]; selectedId: string | null; dispatch: Dispatch<PrototypeAction> };

function BriefingSection({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  return <section className="briefing-section"><header><h2>{title}</h2><span>{count}</span></header><div className="briefing-list">{children}</div></section>;
}

function FixtureState({ state }: { state: Exclude<PrototypeFixtureState, "ready"> }) {
  if (state === "loading") return <div className="fixture-state loading-state" role="status" aria-live="polite"><LunaMark /><h2>I’m putting your briefing together…</h2><p>Reviewing recent Household Work and the information available to me.</p><div className="skeleton-lines"><i/><i/><i/></div></div>;
  if (state === "empty") return <div className="fixture-state"><span className="state-icon"><AppIcon name="check" /></span><h2>Everything is taken care of.</h2><p>I found nothing that needs your attention. You can still ask me to handle something below.</p></div>;
  return <div className="fixture-state" role="alert"><span className="state-icon error"><AppIcon name="alert" /></span><h2>I couldn’t finish your briefing.</h2><p>Your existing Household Work is safe. I’ll try the unavailable sources again, or you can continue the conversation now.</p><button type="button">Try briefing again</button></div>;
}

export function WebFirstPrototype({ variant, fixtureState = "ready", layoutOverride }: { variant: PrototypeVariantKey; fixtureState?: PrototypeFixtureState; layoutOverride?: "mobile" | "tablet" | "desktop" }) {
  const [state, dispatch] = useReducer(prototypeReducer, undefined, createInitialState);
  const [layout, setLayout] = useState<"mobile" | "tablet" | "desktop">(layoutOverride ?? "desktop");
  useEffect(() => {
    if (layoutOverride) return;
    const update = () => setLayout(layoutModeForWidth(window.innerWidth));
    update(); window.addEventListener("resize", update); return () => window.removeEventListener("resize", update);
  }, [layoutOverride]);
  const selectedWork = state.works.find((work) => work.id === state.selectedWorkId) ?? null;
  const variants = { A: VariantA, B: VariantB, C: VariantC };
  const Variant = variants[variant];

  return (
    <div className={`web-shell variant-${variant.toLowerCase()}`} data-layout={layout}>
      <Sidebar active={state.activeNavigation} onNavigate={(destination) => dispatch({ type: "navigate", destination })} />
      <main className="main-workspace">
        <BriefingHeader onOpenContext={() => dispatch({ type: "toggleContext", open: true })} />
        <div className="workspace-scroll">
          {state.notice && <div className="status-notice" role="status">{state.notice}</div>}
          {fixtureState === "ready" ? <Variant works={state.works.filter((work) => work.status !== "dismissed")} selectedId={state.selectedWorkId} dispatch={dispatch} /> : <FixtureState state={fixtureState} />}
        </div>
        <Composer state={state} dispatch={dispatch} />
      </main>
      {layout === "desktop" && <ContextPanel work={selectedWork} correctionOpen={state.correctionOpen} dispatch={dispatch} />}
      {layout === "tablet" && <button className="tablet-context-trigger" type="button" onClick={() => dispatch({ type: "toggleContext", open: true })}><AppIcon name="details" /> Context</button>}
      {(layout === "mobile" || layout === "tablet") && state.contextOpen && <div className="drawer-backdrop" onClick={() => dispatch({ type: "toggleContext", open: false })}><div onClick={(event) => event.stopPropagation()}><ContextPanel work={selectedWork} correctionOpen={state.correctionOpen} dispatch={dispatch} drawer /></div></div>}
      {layout === "mobile" && <MobileNavigation active={state.activeNavigation} onNavigate={(destination) => dispatch({ type: "navigate", destination })} />}
      <PrototypeSwitcher current={variant} />
    </div>
  );
}
