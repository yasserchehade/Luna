# Roadmap

## Phase 0: Foundation

- Create repository structure.
- Add product and technical documentation.
- Add FastAPI backend skeleton.
- Add Next.js dashboard skeleton.
- Add Docker Compose for local development.
- Draft PostgreSQL schema.

## Phase 1: Bills MVP

- Upload PDF bills and invoices. Initial endpoint and dashboard control are in place.
- Store original documents locally. Initial local storage is in place.
- Extract core invoice fields. Current implementation uses a stub extractor.
- Save extracted bill records. Draft bill persistence is in place.
- Show unpaid, paid, overdue, and upcoming bills.
- Add review and correction workflow.

## Phase 2: Email Ingestion

- Connect email inboxes.
- Detect bill and invoice attachments.
- Queue extraction jobs.
- Track source email metadata.

## Phase 3: Automation and Reminders

- Remind users before due dates.
- Detect overdue obligations.
- Suggest categories and classifications.
- Add human approval flows for sensitive actions.

## Phase 4: Integrations

- MYOB export.
- Bank feed payment matching.
- Cloud document storage.
- Calendar reminders.
- Multi-user family or business workspaces.
