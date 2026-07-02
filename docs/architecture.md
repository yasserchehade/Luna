# Architecture

Luna is a modular monorepo with a FastAPI backend, Next.js frontend, PostgreSQL database, Redis-backed job queue, Celery workers, provider-agnostic AI services, and local-first file storage.

The architecture should support a connected household knowledge graph from day one, even while the first user-facing workflow is bill and invoice processing.

## System Concept

External systems feed data into Luna. Luna stores source material, extracts structured information, maps entities and relationships, indexes documents, creates tasks and reminders, and presents the user with a unified dashboard and natural-language assistant.

```text
External systems
  Email, uploads, cloud storage, calendars, MYOB, banks, school apps, portals
        |
        v
Intake and ingestion
  Documents, events, transactions, account metadata, messages
        |
        v
Understanding layer
  Text extraction, AI parsing, entity extraction, relationship mapping
        |
        v
Household knowledge graph
  Household entities, documents, obligations, tasks, reminders, audit history
        |
        v
User experience
  Dashboard, review queues, reminders, assistant, daily briefing
```

## Components

- Frontend: household dashboard, document upload, bill review, task and reminder views, future assistant interface.
- Backend API: authentication boundary, household entities, documents, bills, tasks, reminders, extraction orchestration, and integration adapters.
- Worker: Celery process for document parsing, AI extraction, relationship mapping, reminders, synchronization, and future integration jobs.
- PostgreSQL: structured source of truth for household entities, documents, relationships, tasks, reminders, extraction runs, and audit events.
- Redis: queue broker and lightweight job coordination.
- File storage: local disk for MVP, abstracted for later S3 or Cloudflare R2.
- AI services: provider-agnostic interfaces for extraction, classification, summarization, relationship mapping, and assistant responses.
- Integration adapters: later connectors for Gmail/Outlook, Google Calendar, MYOB, bank feeds, cloud storage, school systems, and other household services.

## Phase 1 Data Flow

1. A user uploads a bill or invoice PDF.
2. Backend stores the original document outside the web root.
3. Backend records document metadata and queues understanding work.
4. Luna extracts PDF text and stores it as document understanding context.
5. Worker or API flow calls the configured AI extraction service.
6. Luna matches the document against known supplier profiles and records template drift when anchors are missing.
7. Luna creates or links household entities such as supplier, property, utility account, subscription, business, or bill.
8. Luna stores a draft bill or invoice with confidence metadata.
9. Luna creates reminders and review tasks for due dates, missing information, or changed supplier templates.
10. User reviews, corrects, confirms, or marks the obligation as paid.
11. Dashboard shows upcoming, unpaid, overdue, and completed household obligations.

## Design Constraints

- Route handlers should orchestrate work, not contain provider-specific extraction logic.
- AI services should return structured outputs with confidence, provenance, and missing-field markers.
- Integrations should be adapters behind internal interfaces.
- Source documents should remain immutable where possible.
- Relationship mapping should be additive and auditable; the system should explain why it linked a document to an entity.
- The assistant should answer from Luna's structured data and indexed documents, not from ungrounded guesses.

## Future Integration Points

- Gmail and Outlook for automatic document intake.
- Google Calendar and Outlook Calendar for household deadlines.
- MYOB for accounting context and export.
- Bank feeds for payment matching.
- Cloud storage for source documents and household records.
- School, insurance, government, and utility portals where APIs or email workflows allow.
