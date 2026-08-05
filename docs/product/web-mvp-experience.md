# Luna web MVP experience

**Status:** Governing web product experience

## Founder acceptance

On 5 August 2026, the founder selected **Variant A — Briefing stream** as the structural direction for Luna's web MVP. It is the accepted foundation because it makes the proactive `Today` briefing primary, separates work requiring attention from upcoming and completed outcomes, and preserves conversation and working context without turning the experience into a dashboard.

The three-variant prototype remains primary-source design evidence on branch `codex/web-first-mvp-prototype`, starting at commit `3047f435d214c3e59bd0469873acea704176e289`. Future implementation must promote Variant A into a real, production-quality route rather than merging the throwaway switcher or the losing Variant B and C implementations into the product.

## Production frontend status

Variant A is promoted into the production frontend foundation at `/today` on branch `codex/promote-variant-a-today`. The root web route redirects to `Today`. The production route has no variant switcher, founder-review controls, query-driven fixture states or visible implementation notes. The historical three-variant prototype remains available only at its explicit `/prototype/web-first` path as design evidence.

The route currently consumes the web-facing `TodayService` interface through an in-browser mock adapter. The adapter owns fixture briefing data and local mutation behavior for approval, correction, dismissal, completion, attachment and conversation. UI modules do not import fixture data directly and do not copy Rust domain types. This is a frontend contract and interaction foundation, not evidence of production authentication, persistence, background review, OpenAI, email or storage connectivity.

`TodayService` exposes one chronological household conversation. Messages may cite zero, one or several contextual Household Work identifiers, and a conversational result identifies every affected work item or a clarification request. Household Work does not own a separate per-report conversation. The mock adapter resolves explicit work references first, treats selected work only as an optional hint, preserves unrelated work and asks a clarifying question when a consequential reference is ambiguous.

Backend integration remains blocked on founder acceptance of the production route. Once accepted, the next implementation design should map the approved web-facing contracts to a minimal authenticated household service without changing the accepted information hierarchy.

## Experience promise

Opening Luna should feel like checking in with a competent household employee who has already been working. Luna starts the conversation with a concise briefing, presents household outcomes in plain language and keeps a natural conversation available without making the member operate a workflow.

The experience is not a generic chatbot, document manager, email client, metrics dashboard, workflow builder or task-management application. The current desktop interface must not be recreated in a browser.

## Primary journey

1. The member opens Luna on `Today`.
2. Luna states what it reviewed, what it completed, what needs the member and what is upcoming.
3. Matters appear as structured Household Work reports inside the briefing or conversation.
4. Selecting work updates a concise visible working context without changing the conversation's scope.
5. The member approves, corrects, dismisses, completes or discusses the matter naturally.
6. The persistent composer remains global by default and may show a removable Household Work, household entity or attached-source context hint.
7. Detailed evidence and audit information are available only when requested.

The first prototype uses clearly identified fixture data. It does not imply that background review, email intake or external execution already exists.

## Information architecture

The primary navigation is:

- **Today** — the default briefing and active conversation;
- **Conversations** — durable member dialogue organised around household matters;
- **Calendar** — upcoming household obligations and, later, authorised calendar sources/actions;
- **Cabinet** — the logical source and document layer, not a local path browser;
- **Household** — members, properties, accounts, providers and responsibilities;
- **History** — completed work, decisions and outcomes;
- **Settings** — household and service preferences that do not belong in the primary experience.

There is no `Dashboard`. Navigation labels describe household outcomes rather than implementation systems.

## Today workspace

`Today` begins with a time-aware greeting and a short proactive briefing, for example:

> Good evening, Yasser.
>
> While you were away I reviewed 24 new emails, two documents and your calendar.
>
> Here is what needs your attention today.

The briefing distinguishes:

- work completed while the member was away;
- work requiring attention, clarification or approval;
- upcoming obligations; and
- whether anything else needs attention.

Counts may help scan a section but must not become KPI tiles. Empty state is a positive outcome: Luna says that everything is taken care of and leaves the composer available. Loading and failure states explain what is happening and explicitly preserve existing Household Work.

## Household Work reports

A report may contain:

- a member-facing title and concise summary;
- source and source type;
- due date or timing;
- relevant household entity;
- Luna's recommendation;
- approval, discuss, completion or dismissal actions; and
- a route to source/evidence details.

Reports are structured parts of the conversation, not generic dashboard cards. They never expose internal state-machine names, model identifiers, field schemas or workflow controls. A member correction becomes conversational direction and must preserve unrelated facts and history after Luna-owned validation.

## Working-context panel

On desktop, the right panel shows only the selected work's current context. Selection controls what the member is viewing; it does not create a separate conversation or require subsequent messages to target that work:

- what Luna is currently working on;
- the relevant source;
- the related household entity;
- facts Luna understands;
- what Luna still needs; and
- the recommended or proposed action.

Detailed evidence, extracted fields, source metadata, audit events and previous interpretations stay behind `View source and evidence`. Model reasoning traces are never exposed.

On tablet the panel becomes a drawer. On mobile it is a work-details view opened from the briefing. The full three-column desktop layout is never squeezed onto mobile.

## Persistent composer

The composer remains anchored at the bottom of the main workspace and is visually a conversation input, not a search field. Its default invitation is:

> What would you like me to take care of?

It supports natural language and PDF/JPG/PNG attachment selection in the prototype. The composer starts without a selected target. When work or a source is relevant, a compact removable context token presents it as a routing hint; removing that token preserves both the draft and the work being viewed. Explicit language can target other work, global questions remain household-wide, and ambiguous consequential instructions produce clarification instead of mutation. The design reserves space for future voice without implementing voice controls now.

The composer remains available in ready, empty, loading and recoverable-failure states.

## Responsive behavior

### Desktop

- persistent left navigation;
- central briefing/conversation workspace;
- persistent right working-context panel; and
- bottom-anchored composer inside the central workspace.

### Tablet

- left navigation may remain compact;
- central briefing remains primary;
- working context opens as a drawer or secondary view; and
- composer remains anchored.

### Mobile

- compact bottom navigation;
- one-column briefing and conversation feed;
- bottom-anchored composer above navigation; and
- contextual information through a work-details drawer/view.

## Visual and language direction

- light, spacious and professional;
- restrained blue accent with sufficient contrast;
- subtle borders and limited shadows;
- no decorative gradients or excessive rounded containers;
- calm information density inspired structurally by active-work tools, without developer terminology or copied branding;
- clear focus states, semantic regions, labelled controls and status text; and
- no essential meaning conveyed only by colour.

Luna speaks in outcomes: what it found, why it matters, what it handled, what it recommends and what it genuinely needs from the member.

## Frontend states and interactions

The production route demonstrates through the mock adapter:

- selecting navigation and Household Work;
- context-panel updates;
- one continuous household conversation across all reports;
- global conversation without preselection, optional removable context hints and clarification for ambiguous references;
- explicit and multi-work conversational updates without duplicating Household Work;
- approval, discussion, dismissal and completion;
- correction without replacing unrelated information;
- attachment selection without upload;
- desktop, tablet and mobile layouts;
- loading, empty and failure briefings; and
- visible disclosure that all mutations are local mock state.

It deliberately excludes authentication, persistence, live OpenAI calls, connectors, background work and production briefing generation. Loading uses a structured skeleton; empty, unavailable, recoverable-error and partial-failure states preserve the composer and distinguish no work from unavailable work.
