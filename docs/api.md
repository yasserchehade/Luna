# API

The API starts small and should grow around real MVP workflows.

## Current Endpoints

- `GET /health`: service health check.
- `POST /api/documents`: upload a PDF bill or invoice and create a stored document record.
- `GET /api/bills`: dashboard bill list from persisted bill records.
- `POST /api/bills/ingest`: accepts a stored document id, creates an extraction run, and saves a draft bill.

## Planned MVP Endpoints

- `GET /api/documents/{id}`: retrieve document metadata.
- `GET /api/bills`: list bills by status, due date, category, and classification.
- `PATCH /api/bills/{id}`: correct extracted fields.
- `POST /api/bills/{id}/confirm`: approve extracted draft data.
- `POST /api/bills/{id}/mark-paid`: record a manual payment status.

## API Principles

- AI extraction output is draft data until confirmed.
- Financial actions must require explicit approval.
- Document ids should remain stable because original documents are the source of truth.
- Future integrations should use service adapters rather than provider-specific logic in route handlers.
