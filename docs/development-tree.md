# Development Tree

Project Luna should evolve through vertical tracer bullets. Each issue should teach Luna one new household-employee responsibility and deliver it end to end across data model, backend, API, frontend, tests, audit, and documentation.

Do not build horizontally by finishing all database work, then all backend work, then all UI work. A completed issue should be user-visible and safe to reason about.

## Active Product Language

- Luna, not Assistant.
- Workbench, not Dashboard.
- Obligations, not Bills as the whole product.
- Household, not generic workspace.
- Approvals, not simple task completion.

## Phase 0: Alignment and Safety

Goal: make the repo safe to evolve around Luna as an employee.

Status: active.

- Keep `PRD.md`, `docs/authority.md`, and `docs/product-refocus.md` as product anchors.
- Preserve existing code; do not delete or broadly restructure yet.
- Use vertical tracer bullets for implementation.
- Keep original documents immutable where possible.
- Treat AI output as draft until confirmed.
- Route household-record changes through work order, authority, approval, and audit.

## Phase 1: Foundational Contract

Goal: give Luna an employment contract: authority, work orders, approvals, and audit.

Tracer Bullet 1: Luna can prepare a bill for approval.

This is the first implementation priority. It proves that Luna can receive a bill, extract important information, prepare work, request approval where required, show that request in the Workbench, record the decision, and avoid external execution.

## Phase 2: Records Responsibility

Tracer Bullet 2: Luna can prepare a document filing suggestion.

Luna should prepare a cabinet path suggestion with reasoning, request approval before final filing changes, preserve the original document, and audit approval or rejection.

## Phase 3: Obligations Responsibility

Tracer Bullet 3: Luna can turn a bill into an obligation with reminders.

Luna should create or update obligations from confirmed bills, monitor due dates, create reminders, surface upcoming and overdue obligations, and audit changes.

## Phase 4: Household Knowledge Responsibility

Tracer Bullet 4: Luna can suggest household graph relationships for approval.

Luna should suggest relationships with evidence and confidence, update the graph only after approval, and preserve rejected suggestions in audit history.

## Phase 5: Workbench Responsibility

Tracer Bullet 5: Luna Workbench shows what needs attention today.

The main screen should normalize tasks, approvals, obligations, risks, prepared work, and recently completed work into a household-operations view.

## Phase 6: Luna Conversation Responsibility

Tracer Bullet 6: Luna can answer questions using household records.

Answers must be grounded in structured records and documents, cite sources where possible, show uncertainty, and avoid generic chatbot behavior.

## Phase 7: Onboarding Responsibility

Tracer Bullet 7: Luna can onboard a household gradually.

Onboarding should establish household owner, members, roles, responsibilities, initial authority policies, and first Workbench state.

## Phase 8: Read-Only Observation

Tracer Bullet 8: Luna can register a read-only connection scope.

Connection scopes should make read, write, and execute permissions visible before real external integrations are added.

## Phase 9: Specialist Agent Architecture

Tracer Bullet 9: Luna can route work to specialist services internally.

Specialists stay internal; the household interacts with one Luna.

## Phase 10: Trusted Employee Mode

Tracer Bullet 10: Luna can automatically perform low-risk internal work.

Do not build until assisted workflows are trusted. Automatic work requires explicit delegated authority, confidence thresholds, escalation rules, correction paths, and audit.

## Phase 11: Future Execution

Future external execution is out of MVP scope. Bill payment, booking, renewals, email sending, legal filing, bank actions, MYOB write actions, and other consequential external actions require a mature authority model, production security review, real integrations, error recovery, and legal/compliance review.

## Parallel Work Rules

Can be parallel after Phase 1 foundations:

- Cabinet filing workflow.
- Graph suggestion workflow.
- Workbench UI polish.
- Audit display.
- UX copy updates.
- Documentation updates.

Should not be parallel before foundations:

- External integrations.
- Autonomous execution.
- Banking actions.
- MYOB write actions.
- Payment workflows.
- Email sending.

Dependency rule: if a task changes household records, graph, obligations, storage, approvals, or authority, it must go through work order, authority, and audit.
