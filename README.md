# Luna

Luna is a Household Operating System: an intelligence and coordination layer for modern family life.

Families already use banking apps, MYOB, calendars, email, school portals, insurance portals, cloud storage, government systems, and utility providers. Luna is not trying to replace those systems. Luna sits above them, connects what they know, understands the relationships between household information, and helps the family manage the administrative work through one intelligent interface.

The core promise is simple: people should not have to remember where information is stored. They should only have to remember Luna.

## Product Direction

Luna starts with bills and invoices because they are frequent, document-heavy, deadline-driven, and painful enough to prove the system. The architecture is broader from day one: documents become structured entities, entities are connected into a household knowledge graph, and Luna uses that graph to create reminders, tasks, dashboards, and proactive briefings.

Over time Luna should coordinate:

- Bills, invoices, payments, and subscriptions.
- Properties, utilities, insurance, warranties, and assets.
- Vehicles, registrations, servicing, and policies.
- School obligations, family calendar events, and health appointments.
- Government documents, tax deadlines, and cloud documents.
- Banking and MYOB/accounting context through integrations.

## Stack

- Backend: Python, FastAPI
- Frontend: Next.js, React, TypeScript
- Database: PostgreSQL
- Background jobs: Celery, Redis
- File storage: local-first user-owned cabinet storage, with optional user cloud folders or Luna encrypted cloud later
- AI: provider-agnostic service interfaces
- Containers: Docker Compose

## Architecture Summary

External systems feed documents, events, transactions, and metadata into Luna. Luna stores or references the original source material in a user-owned household cabinet, extracts structured information, maps entities and relationships, indexes documents, creates tasks and reminders, and presents focused views for the dashboard, cabinet, household structure, and future natural-language assistant.

The MVP begins with uploaded invoices and bills:

1. Store or reference the original document in the household cabinet.
2. Extract supplier, amount, due date, invoice number, and related metadata.
3. Attach the document to household entities such as supplier, property, utility account, subscription, or business.
4. Create reminders and review tasks.
5. Show urgent obligations in a dashboard while keeping the household cabinet and structure available in separate views.

## Local Development

1. Copy environment files if you want to customize defaults:

```powershell
Copy-Item backend/.env.example backend/.env
Copy-Item frontend/.env.example frontend/.env.local
```

2. Install backend dependencies:

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e .\backend[dev]
```

3. Install frontend dependencies:

```powershell
pnpm install
```

If Node and pnpm are only available through the Codex bundled runtime, add the bundled Node directory to PATH before running frontend commands:

```powershell
$env:Path = "C:\Users\yasse\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin;" + $env:Path
C:\Users\yasse\.cache\codex-runtimes\codex-primary-runtime\dependencies\bin\pnpm.cmd install
```

4. Start the local Docker stack when Docker Desktop is installed:

```powershell
docker compose up --build
```

5. Open the apps:

- Frontend: http://localhost:3000
- Backend API: http://localhost:8000
- API docs: http://localhost:8000/docs

For backend-only development without Docker:

```powershell
.\.venv\Scripts\python.exe -m uvicorn app.main:app --app-dir backend --reload
```

## Repository Structure

```text
Luna/
  docs/              Product, architecture, database, AI, graph, and security notes
  backend/           FastAPI API, extraction service, storage service, DB schema
  frontend/          Next.js household operating system shell
  infrastructure/    PostgreSQL schema and deployment-facing assets
  docker/            Dockerfiles
  .github/           CI workflows
```

## Current MVP Surface

- `GET /health` returns service status.
- `POST /api/documents` accepts PDF bill or invoice uploads and stores the original file in local cabinet storage.
- `GET /api/bills` returns persisted bill records from PostgreSQL.
- `GET /api/documents` returns recent document records with cabinet planning status.
- `POST /api/bills/ingest` accepts a stored document id, runs the current extractor, records the extraction run, and saves a draft bill.
- `POST /api/documents/{id}/cabinet-file` copies or moves a confirmed document into the configured local cabinet root.
- `POST /api/knowledge/ask` answers simple household questions from structured Luna records and source references.
- `GET /api/audit-events` returns recent audit history for important document, bill, graph, task, reminder, and assistant actions.
- The frontend has separate Dashboard, Cabinet, Structure, and Assistant tabs so active obligations, archived documents, household graph setup, and grounded questions do not compete for attention.
- The Dashboard and Cabinet tabs expose basic action controls for confirming bills, marking bills paid, archiving bills, planning cabinet paths, and confirming cabinet paths.

The extraction implementation is intentionally a stub behind an interface so OpenAI, Claude, Gemini, or another provider can be added later without changing the API layer.

The Celery worker is wired into Docker Compose but does not perform real extraction yet. That keeps background processing ready without overbuilding before document ingestion exists.

## Documentation

- [Vision](docs/vision.md)
- [Principles](docs/principles.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Database](docs/database.md)
- [Storage](docs/storage.md)
- [AI Agents](docs/ai-agents.md)
- [Security](docs/security.md)
- [Product Positioning](docs/product-positioning.md)
- [Knowledge Graph](docs/knowledge-graph.md)
