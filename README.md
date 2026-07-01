# Luna

Luna is an AI Administrative Operating System for bills, invoices, renewals, registrations, property expenses, tax deadlines, documents, emails, and reminders.

The first MVP focuses on ingesting bills and invoices, extracting key fields with an AI provider-agnostic interface, storing the document and structured data, and showing unpaid, paid, overdue, and upcoming bills in a dashboard.

## Stack

- Backend: Python, FastAPI
- Frontend: Next.js, React, TypeScript
- Database: PostgreSQL
- Background jobs: Celery, Redis
- File storage: local filesystem first, S3 or Cloudflare R2 later
- AI extraction: provider-agnostic adapter interface
- Containers: Docker Compose

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
  docs/              Product, architecture, database, AI, and security notes
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
