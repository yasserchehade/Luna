# Architecture

Luna is a modular monorepo with a FastAPI backend, Next.js frontend, PostgreSQL database, Redis-backed job queue, Celery workers, provider-agnostic AI services, authority controls, and local-first user-owned cabinet storage.

The architecture should support Luna as an AI household employee from day one, even while the first user-facing workflow is records, documents, obligations, and bill processing.

## System Concept

External systems feed data into Luna through explicit connections. Luna stores or references source material in a user-owned household cabinet, extracts structured information, maps entities and relationships, checks authority, indexes documents, creates tasks and reminders, prepares work for approval, and presents the household with one employee-like interface.

```text
External systems
  Email, uploads, cloud storage, calendars, MYOB, banks, school apps, portals
        |
        v
Connections and intake
  Read, write, or execute scopes; documents, events, transactions, account metadata, messages
        |
        v
Understanding layer
  Text extraction, AI parsing, entity extraction, relationship mapping
        |
        v
Household memory and authority
  Household entities, documents, obligations, tasks, reminders, authority contract, audit history
        |
        v
Work orchestration
  Observed, prepared, proposed, approval requested, approved, executed, escalated
        |
        v
User experience
  Luna employee interface, dashboard, cabinet, structure graph, approvals, reminders, daily briefing
```

## Components

- Frontend: focused Dashboard for current attention, Cabinet for document archive access, Structure for household graph setup, assisted work and approval queues, plus future bill review, task, reminder, and employee-style Luna interfaces.
- Backend API: authentication boundary, household entities, member roles, authority policies, documents, bills, tasks, reminders, approval requests, extraction orchestration, and integration adapters.
- Worker: Celery process for document parsing, AI extraction, relationship mapping, reminders, synchronization, work preparation, and future integration jobs.
- PostgreSQL: structured source of truth for household entities, member roles, authority policies, connection scopes, documents, relationships, obligations, tasks, reminders, work orders, approval requests, extraction runs, and audit events.
- Redis: queue broker and lightweight job coordination.
- File storage: provider-aware household cabinet storage. Local folder first, later user-owned cloud folders such as iCloud Drive, Google Drive, OneDrive, Dropbox, or NAS. Luna-managed encrypted cloud storage should be optional.
- AI services: provider-agnostic interfaces for extraction, classification, summarization, relationship mapping, briefing generation, and specialist-agent work behind the single Luna interface.
- Integration adapters: later connectors for Gmail/Outlook, Google Calendar, MYOB, bank feeds, cloud storage, school systems, and other household services.
- Authority layer: shared policy and approval checks that every write or execute action must pass before touching internal records or external services.

## Phase 1 Data Flow

1. A household administrator or authorised member uploads a bill or invoice PDF.
2. Backend stores the original document in local cabinet storage outside the web root.
3. Backend records document metadata and queues understanding work.
4. Luna extracts PDF text and stores it as document understanding context.
5. Worker or API flow calls the configured AI extraction service.
6. Luna matches the document against known supplier profiles and records template drift when anchors are missing.
7. Luna creates or links household entities such as supplier, property, utility account, subscription, business, or bill.
8. Luna generates a suggested cabinet path from the graph and extracted document context.
9. Luna stores a draft bill or invoice with confidence metadata.
10. Luna creates reminders and review or approval tasks for due dates, missing information, changed supplier templates, or authority-restricted work.
11. User reviews, corrects, confirms, approves, files, or marks the obligation as paid.
12. Dashboard shows upcoming, unpaid, overdue, and completed household obligations.

Phase 1 should not execute payments, send external messages, renew policies, book services, or make irreversible changes. It may model those as future work types and stop at proposed or approval requested.

## Household Cabinet Storage

The household cabinet is a file structure the user can access outside Luna. It may live on a local drive, external drive, NAS, iCloud Drive, Google Drive, OneDrive, Dropbox, or another user-owned storage location.

The knowledge graph is the source of truth. Cabinet folders are a human-readable projection of that graph. For example, a user-defined graph such as `FamilyMember -> FamilyTrust -> Property -> Supplier -> Document` can produce cabinet paths under a family trust, property, supplier, or document category.

For MVP, Luna should generate suggested paths before physically moving or renaming files. This lets the product prove filing intelligence without risking silent destructive changes.

## Design Constraints

- Route handlers should orchestrate work, not contain provider-specific extraction logic.
- AI services should return structured outputs with confidence, provenance, and missing-field markers.
- Integrations should be adapters behind internal interfaces.
- Every integration capability should declare whether it is read, write, or execute.
- Write and execute capabilities should call the authority layer before changing records or external systems.
- Source documents should remain immutable where possible.
- Luna should not silently delete, overwrite, or irreversibly move user files.
- Relationship mapping should be additive and auditable; the system should explain why it linked a document to an entity.
- Luna should answer and act from structured data, authority state, and indexed documents, not from ungrounded guesses.

## Future Integration Points

- Gmail and Outlook for automatic document intake.
- Google Calendar and Outlook Calendar for household deadlines.
- MYOB for accounting context and export.
- Bank feeds for payment matching.
- User-owned cloud folders for source documents and household records.
- Optional Luna encrypted cloud sync or backup for households that want managed convenience.
- School, insurance, government, and utility portals where APIs or email workflows allow.
- Approval-controlled write and execute actions such as prepared emails, draft calendar events, bill payments, bookings, renewals, cancellations, and accountant-ready exports.
