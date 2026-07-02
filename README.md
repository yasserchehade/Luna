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
- File storage: local filesystem first, S3 or Cloudflare R2 later
- AI: provider-agnostic service interfaces
- Containers: Docker Compose

## Architecture Summary

External systems feed documents, events, transactions, and metadata into Luna. Luna stores the original source material, extracts structured information, maps entities and relationships, indexes documents, creates tasks and reminders, and presents a unified dashboard plus a natural-language assistant.

The MVP begins with uploaded invoices and bills:

1. Store the original document.
2. Extract supplier, amount, due date, invoice number, and related metadata.
3. Attach the document to household entities such as supplier, property, utility account, subscription, or business.
4. Create reminders and review tasks.
5. Show upcoming, unpaid, overdue, and completed obligations in a dashboard.

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
  frontend/          Next.js dashboard shell
  infrastructure/    PostgreSQL schema and deployment-facing assets
  docker/            Dockerfiles
  .github/           CI workflows
```

## Current MVP Surface

- `GET /health` returns service status.
- `POST /api/documents` accepts PDF bill or invoice uploads and stores the original file locally.
- `GET /api/bills` returns persisted bill records from PostgreSQL.
- `POST /api/bills/ingest` accepts a stored document id, runs the current extractor, records the extraction run, and saves a draft bill.
- The dashboard can upload a PDF, trigger stub ingestion, and show persisted draft bills.

The extraction implementation is intentionally a stub behind an interface so OpenAI, Claude, Gemini, or another provider can be added later without changing the API layer.

The Celery worker is wired into Docker Compose but does not perform real extraction yet. That keeps background processing ready without overbuilding before document ingestion exists.

## Documentation

- [Vision](docs/vision.md)
- [Principles](docs/principles.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Database](docs/database.md)
- [AI Agents](docs/ai-agents.md)
- [Security](docs/security.md)
- [Product Positioning](docs/product-positioning.md)
- [Knowledge Graph](docs/knowledge-graph.md)
