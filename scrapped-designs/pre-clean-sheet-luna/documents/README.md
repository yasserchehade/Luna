# Luna

Luna is an AI household employee: an intelligence, authority, and coordination layer for modern family life.

Families already use banking apps, MYOB, calendars, email, accountants, school portals, insurance portals, cloud storage, government systems, and utility providers. Luna is not trying to replace those systems. Luna sits above them, connects what they know, understands the relationships between household information, and helps the family manage administrative work through one employee-like interface.

The core mission is simple: eliminate repetitive household administrative work that does not add value to the family's life.

## Product Direction

Luna starts with household records, documents, obligations, bills, and assisted workflows because they are frequent, document-heavy, deadline-driven, authority-sensitive, and painful enough to prove the system. The architecture is broader from day one: documents become structured entities, entities are connected into a household knowledge graph, and Luna uses that graph plus an authority contract to create reminders, approval requests, tasks, dashboards, and proactive briefings.

Over time Luna should coordinate:

- Bills, invoices, payments, and subscriptions.
- Properties, utilities, insurance, warranties, and assets.
- Vehicles, registrations, servicing, and policies.
- School obligations, family calendar events, and health appointments.
- Government documents, tax deadlines, and cloud documents.
- Banking and MYOB/accounting context through integrations.
- Routine accounts payable, accounts receivable, bank reconciliation, and reporting preparation where authorised.

Luna is public-facing as one employee. Internally, specialist agents may handle finance, documents, property, vehicles, insurance, scheduling, communications, and compliance work, but the household interacts with Luna only.

## Authority Model

Luna's actions depend on the household employment contract. The contract defines what Luna may read, write, execute, who can approve, what spending limits apply, and when work must be escalated.

External connections have three levels:

1. Read: Luna can observe and retrieve information.
2. Write: Luna can create or modify records, drafts, events, folders, or similar objects.
3. Execute: Luna can perform consequential actions such as paying bills, booking services, renewing policies, or cancelling subscriptions.

The MVP is assisted, not autonomous: Luna prepares and requests approval. Real-world execution remains disabled until the authority model, approvals, and audit trails are mature.

## Stack

- Backend: Python, FastAPI
- Frontend: Next.js, React, TypeScript
- Database: PostgreSQL
- Background jobs: Celery, Redis
- File storage: local-first user-owned cabinet storage, with optional user cloud folders or Luna encrypted cloud later
- AI: provider-agnostic service interfaces with specialist agents behind one Luna interface
- Authority: household roles, approval workflows, connection scopes, and audit trails
- Containers: Docker Compose

## Architecture Summary

External systems feed documents, events, transactions, and metadata into Luna through explicit read, write, or execute connections. Luna stores or references the original source material in a user-owned household cabinet, extracts structured information, maps entities and relationships, checks authority, indexes documents, creates tasks and reminders, prepares work for approval, and presents focused views for the dashboard, cabinet, household structure, approvals, and Luna employee interface.

The MVP begins with uploaded invoices and bills:

1. Store or reference the original document in the household cabinet.
2. Extract supplier, amount, due date, invoice number, and related metadata.
3. Attach the document to household entities such as supplier, property, utility account, subscription, or business.
4. Create reminders and review tasks.
5. Request approval where authority requires it.
6. Show urgent obligations in a dashboard while keeping the household cabinet and structure available in separate views.

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
- Authority is not yet first-class in the schema or UI. The next architectural step is to add household roles, connection scopes, approval requests, work orders, and authority-aware audit events before enabling any write or execute integrations.

The extraction implementation is intentionally a stub behind an interface so OpenAI, Claude, Gemini, or another provider can be added later without changing the API layer.

The Celery worker is wired into Docker Compose but does not perform real extraction yet. That keeps background processing ready without overbuilding before document ingestion exists.

## Documentation

- [Vision](docs/vision.md)
- [Principles](docs/principles.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Product Refocus](docs/product-refocus.md)
- [Database](docs/database.md)
- [Storage](docs/storage.md)
- [AI Agents](docs/ai-agents.md)
- [Authority Model](docs/authority.md)
- [Security](docs/security.md)
- [Product Positioning](docs/product-positioning.md)
- [Knowledge Graph](docs/knowledge-graph.md)
