# Roadmap

## Phase 0: Foundation

- Create repository structure.
- Add product and technical documentation.
- Add FastAPI backend skeleton.
- Add Next.js dashboard skeleton.
- Add Docker Compose for local development.
- Draft PostgreSQL schema.
- Establish provider-agnostic AI extraction boundary.

## Phase 1: Household Bills MVP

Phase 1 should prove Luna as a Household Operating System while staying focused on one buildable workflow: bills and invoices.

- Upload PDF bills and invoices.
- Store original documents in local user-owned cabinet storage.
- Generate suggested cabinet paths from extracted fields and graph relationships.
- Extract supplier, amount, due date, invoice number, category, confidence metadata, and human-review reasons.
- Save draft bill and invoice records in PostgreSQL.
- Assign documents and obligations to household entities such as supplier, property, business, utility account, subscription, or family member where available.
- Create reminders before due dates.
- Create review tasks for missing, conflicting, changed-template, or low-confidence extraction results.
- Show a dashboard for upcoming, unpaid, paid, overdue, and needs-review obligations.
- Keep the extraction implementation behind a provider-agnostic interface.

## Phase 2: Knowledge Graph Foundations

- Add first-class household, family member, supplier, property, vehicle, business, account, and task models.
- Add relationship records so documents can link to multiple entities with provenance and confidence.
- Add provider-aware storage metadata for local folders and future user-owned cloud folders.
- Add safe filing workflows that suggest, confirm, then move or copy files into the household cabinet.
- Add document search and basic indexing.
- Add a natural-language question interface grounded in structured data and documents.
- Add audit events for document views, corrections, relationship edits, and AI suggestions.

## Phase 3: Automated Intake

- Connect Gmail or Outlook for bill and invoice detection.
- Track source email metadata and attachments.
- Add duplicate detection across uploads and email.
- Add user-owned cloud folder import or backup.
- Queue all ingestion and extraction work through Celery.

## Phase 4: Proactive Household Assistant

- Add daily and weekly briefings.
- Detect overdue obligations and missing payment status.
- Suggest tasks based on documents, deadlines, and household context.
- Add calendar reminders and task completion workflows.
- Expand the assistant from Q&A into action preparation with human approval.

## Phase 5: Financial and Accounting Context

- Add bank feed or transaction import adapters for payment matching.
- Add MYOB/accounting context for business and tax workflows.
- Reconcile bills, invoices, subscriptions, and payments.
- Prepare exportable records for accountants or household review.

## Phase 6: Broader Household Administration

- Add insurance, utilities, vehicles, warranties, assets, school obligations, health events, government documents, and tax obligations as first-class workflows.
- Add role-based family access.
- Add retention, deletion, and export workflows.
- Add more integrations where they reduce manual household admin without replacing source systems.
