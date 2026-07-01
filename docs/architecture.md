# Architecture

Luna starts as a modular monorepo with a FastAPI backend, Next.js frontend, PostgreSQL database, Redis-backed job queue, and local file storage.

## Components

- Frontend: dashboard for bill status, review queues, and document views.
- Backend API: authentication, document ingestion, bill records, extraction orchestration, and integrations.
- Worker: Celery process for email sync, PDF parsing, AI extraction, reminders, and future integrations.
- PostgreSQL: structured source of truth for documents, bills, users, classifications, and audit records.
- Redis: queue broker and lightweight job coordination.
- File storage: local disk for MVP, later abstracted to S3 or Cloudflare R2.
- AI extraction providers: interchangeable adapters behind a shared interface.

## MVP Data Flow

1. User uploads a PDF or Luna receives an email attachment.
2. Backend stores the original document.
3. Backend queues extraction work.
4. Worker extracts text and calls the configured AI extraction provider.
5. Extracted fields are saved as a draft bill.
6. User reviews and confirms the bill.
7. Dashboard shows unpaid, paid, overdue, and upcoming obligations.

## Future Integration Points

- MYOB for accounting export and reconciliation.
- Bank feeds for payment matching.
- Calendar providers for reminders.
- Cloud storage for document backup.
- Email providers for automatic bill ingestion.
