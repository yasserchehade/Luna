# MVP reset follow-up implementation issues

**Status:** Web-first sequence under ADR 0020; GitHub issues not yet created

This sequence replaces the desktop and email-first order for future planning. The historical ticket map and the 2026-08-03 MVP reset assessment remain evidence, not the active delivery sequence. Gmail, Outlook and cloud-storage providers are not dependencies of the first web Household Work journey.

## Sequence

### 1. Select the web Today experience

Review the three `apps/web` variants across desktop, tablet and mobile. Choose the briefing hierarchy, persistent-composer behavior and working-context disclosure. Record the selected variant and rationale, then rewrite it as the real route without carrying prototype variants or the switcher into production.

### 2. Define the multi-user web API boundary

Specify authenticated briefing projections, Conversation turns, Household Work commands and bounded uploads. Define household authorization, optimistic concurrency, idempotency, audit, source identity and recoverable failure behavior before choosing which desktop modules to expose or port.

### 3. Resolve and extract PR #76 reusable capability

Reproduce the live clarification, correction and image-delegation failures. Extract only reviewed Household Work rules, strict intelligence schemas, bounded source transport, no-op/terminal invariants and deterministic tests. Do not merge PR #76 into the web frontend and do not preserve local SQLite, Tauri or credential-vault assumptions by default.

### 4. Implement server-owned Household Work and Conversation

Create the minimal shared service for durable Household Work, source references, relevant conversation, facts, responsibility, due date, urgency, lifecycle, proposed action, approval and audit. The browser receives authorised projections and submits commands; it never owns the durable lifecycle.

### 5. Connect one bounded uploaded-document journey

Replace fixture behavior for one path: a member uploads a PDF/JPG/PNG and says, "Take care of this." The service assembles authorised context, calls the managed OpenAI route, validates the proposal and creates or updates one durable work item without asking for known facts.

### 6. Complete conversational work handling

Prove read-only questions, one focused clarification, correction, approval, dismissal, completion and source-linked updates through the web seam. Preserve one Household Work item and keep structured evidence secondary.

### 7. Add safe proposals and smallest useful execution

Implement one approved reminder or draft-reply path behind Luna-owned authority, validation, idempotency, audit and recovery. OpenAI proposes; Luna executes.

### 8. Prove persistence, resilience and responsive accessibility

Cover reload/session continuity, concurrent member commands, duplicate prevention, unavailable intelligence, failed execution, mobile/tablet layouts, keyboard navigation, focus, contrast and member-facing recovery language.

### 9. Design proactive briefing generation

Only after persisted work is trustworthy, define how `Today` projects completed, attention and upcoming work. Background workers and monitoring belong to this separate task; fixture language must not be mistaken for implemented automation.

### 10. Add one source or storage connector

Choose either one incoming communication source or one user-controlled storage provider based on product evidence. Specify competency strengthened, member effort reduced, OAuth/credential boundary, source identity and retry behavior. Do not create a generic connector framework or begin Gmail, Google Drive, OneDrive and Dropbox together.

## Dependency rule

An issue is ready only when its product purpose, durable owner, authorised context, authority boundary, multi-user behavior, failure state and member-facing outcome are specified. No issue may add a generic workflow system, connector catalogue, Luna-managed storage or desktop UX as a shortcut around the web Household Work journey.
