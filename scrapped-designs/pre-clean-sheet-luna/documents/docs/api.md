# API

The API starts small and should grow around real MVP workflows.

## Current Endpoints

- `GET /health`: service health check.
- `POST /api/documents`: upload a PDF bill or invoice and create a stored document record.
- `GET /api/documents`: list recent documents with text extraction and cabinet planning status.
- `GET /api/documents/search?query=...`: search recent cabinet documents by filename, cabinet path, bill metadata, and extracted text.
- `GET /api/documents/{id}`: retrieve document metadata including text extraction status.
- `GET /api/documents/{id}/text`: retrieve extracted text for a stored document.
- `POST /api/documents/{id}/cabinet-plan`: generate and save a suggested household cabinet path without moving the file.
- `POST /api/documents/{id}/cabinet-confirm`: confirm the suggested or supplied cabinet path without moving the file.
- `POST /api/documents/{id}/cabinet-file`: explicitly copy or move a confirmed document into the configured local cabinet root and mark it filed.
- `GET /api/bills`: dashboard bill list from persisted bill records, including extraction confidence and review status.
- `POST /api/bills/ingest`: accepts a stored document id, creates an extraction run, saves a draft bill, and stores review reasons when Luna is uncertain. This endpoint is idempotent for documents that already have a bill.
- `PATCH /api/bills/{id}`: correct extracted bill fields.
- `POST /api/bills/{id}/confirm`: confirm draft data, clear review reasons, close open review tasks, and move the bill to unpaid.
- `POST /api/bills/{id}/mark-paid`: record a manual payment status.
- `POST /api/bills/{id}/archive`: archive a bill that is no longer relevant to active workflows.
- `GET /api/household/entities`: list household graph entities.
- `POST /api/household/entities`: create a household graph entity such as a family member, trust, property, supplier, or maintenance item.
- `GET /api/household/entities/{id}/relationships`: inspect all graph relationships attached to one household entity.
- `PATCH /api/household/entities/{id}`: rename, re-type, or replace metadata for a graph entity.
- `GET /api/household/graph`: return graph nodes and relationships for a future visual relationship builder.
- `POST /api/household/relationships`: connect two household entities with a typed relationship.
- `DELETE /api/household/relationships/{id}`: remove an incorrect relationship from the graph.
- `GET /api/household/summary`: returns recent household entities, open tasks, and upcoming reminders.
- `POST /api/household/tasks`: create a household task tied to any graph node.
- `POST /api/household/tasks/{id}/complete`: mark a household task as done.
- `POST /api/household/tasks/{id}/dismiss`: dismiss a household task.
- `POST /api/household/tasks/{id}/archive`: archive a task while preserving graph history.
- `POST /api/household/reminders`: create a scheduled reminder tied to any graph node.
- `POST /api/household/reminders/{id}/dismiss`: dismiss a reminder.
- `POST /api/household/reminders/{id}/archive`: archive a reminder while preserving graph history.
- `POST /api/knowledge/ask`: answer a simple household question from structured Luna records and source references.
- `GET /api/audit-events`: list recent append-only audit events for important user and system actions.

## API Principles

- AI extraction output is draft data until confirmed.
- Financial actions must require explicit approval.
- Document ids should remain stable because original documents are the source of truth.
- Future integrations should use service adapters rather than provider-specific logic in route handlers.
