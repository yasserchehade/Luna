# Roadmap

## Phase 0: Foundation

- Create repository structure.
- Add product and technical documentation.
- Add FastAPI backend skeleton.
- Add Next.js dashboard skeleton.
- Add Docker Compose for local development.
- Draft PostgreSQL schema.
- Establish provider-agnostic AI extraction boundary.

## Phase 1: Household Employee MVP

Status: MVP foundation complete.

Phase 1 proves Luna as an AI household employee while staying focused on one buildable workflow: household records, documents, obligations, and bills.

- Create a household profile with one main household administrator.
- Represent invited household members and simple authority roles: owner, admin, member, viewer.
- Track a minimal authority contract for read, write, and execute boundaries even if execution is disabled in MVP.
- Upload PDF bills and invoices.
- Store original documents in local user-owned cabinet storage.
- Generate suggested cabinet paths from extracted fields and graph relationships.
- Extract supplier, amount, due date, invoice number, category, confidence metadata, and human-review reasons.
- Save draft bill and invoice records in PostgreSQL.
- Assign documents and obligations to household entities such as supplier, property, business, utility account, subscription, or family member where available. Supplier assignment is automated in Phase 1; richer manual graph assignment continues in Phase 2.
- Create reminders before due dates.
- Create review and approval tasks for missing, conflicting, changed-template, low-confidence, or authority-restricted work.
- Show a focused dashboard for upcoming, unpaid, paid, overdue, and needs-review obligations.
- Make assisted work visible: Luna prepared this, here is the evidence, here is the requested approval.
- Keep cabinet archive access and household structure setup in separate views from the daily dashboard.
- Keep the extraction implementation behind a provider-agnostic interface.

Phase 1 does not attempt production-grade auth, payments, cloud sync, real email intake, or autonomous execution. Those belong in later phases after the core document-to-obligation-to-approval workflow is stable.

## Phase 2: Knowledge Graph and Authority Foundations

- Add first-class household, family member, supplier, property, vehicle, business, account, and task models.
- Add explicit authority policy records for member roles, connection scopes, approval rules, spending limits, escalation rules, and work types.
- Add approval request and work order records so Luna can separate prepared work from executed work.
- Add relationship records so documents can link to multiple entities with provenance and confidence.
- Add provider-aware storage metadata for local folders and future user-owned cloud folders.
- Add safe filing workflows that suggest, confirm, then move or copy files into the household cabinet. Initial explicit copy/move filing is in place for confirmed local documents.
- Add document search and basic indexing.
- Add a natural-language question interface grounded in structured data and documents.
- Add audit events for document views, corrections, relationship edits, and AI suggestions. Initial append-only audit logging is in place for key document, bill, graph, task, reminder, and assistant actions.

## Phase 3: Automated Intake

- Connect Gmail or Outlook for bill and invoice detection.
- Model every external connection with read, write, or execute capability levels.
- Track source email metadata and attachments.
- Add duplicate detection across uploads and email.
- Add user-owned cloud folder import or backup.
- Queue all ingestion and extraction work through Celery.

## Phase 4: Proactive Household Assistant

- Add daily and weekly briefings.
- Detect overdue obligations and missing payment status.
- Suggest tasks based on documents, deadlines, and household context.
- Add calendar reminders and task completion workflows.
- Expand Luna from Q&A into action preparation with human approval and explicit authority checks.

## Phase 5: Financial and Accounting Context

- Add bank feed or transaction import adapters for payment matching.
- Add MYOB/accounting context for business and tax workflows.
- Reconcile bills, invoices, subscriptions, and payments.
- Prepare exportable records for accountants or household review.
- Support routine accounts payable, accounts receivable, bank reconciliation, and monthly or quarterly report compilation where authorised.

## Phase 6: Broader Household Administration

- Add insurance, utilities, vehicles, warranties, assets, school obligations, health events, government documents, and tax obligations as first-class workflows.
- Add role-based family access.
- Add retention, deletion, and export workflows.
- Add more integrations where they reduce manual household admin without replacing source systems.
- Add trusted routine execution for carefully scoped, auditable, reversible-where-possible work.
