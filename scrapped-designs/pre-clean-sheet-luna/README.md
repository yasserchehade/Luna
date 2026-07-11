# Scrapped Luna design: pre-clean-sheet iteration

This directory preserves the abandoned Luna implementation and its supporting design documents as they existed when the project restarted from a clean sheet in July 2026.

It is historical reference, not an implementation base. Do not import its architecture, terminology or interfaces into the current Luna build unless a current ticket and ADR explicitly justify reuse.

## Contents

- `implementation/` — the former Python/FastAPI backend, Next.js workbench UI, PostgreSQL and Docker setup, CI workflow and associated local changes.
- `documents/` — the former PRD, roadmap, architecture, product, security, storage and tracer-bullet documents.

The approved domain model, ADRs, clean-sheet tickets, quality gates and Option A UI prototype remain outside this archive. Sample bills also remain in the repository's existing ignored `samples/` directory.
