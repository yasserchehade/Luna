# Luna

Luna is a digital household employee that takes ownership of household administration. It observes incoming household information, understands it in context, creates durable household work, proposes or completes authorised actions, and keeps the household involved only when genuinely necessary.

The default household experience is conversation-first: members delegate in ordinary language, Luna uses relevant email, attachments, household context and recent conversation, and typed work state remains behind the interaction. Structured inspection and correction are available through **Review details** without turning the main experience into a form.

## Current design sources

- `CONTEXT.md` — the domain glossary and preferred language.
- `docs/product/product-constitution.md` — the binding product direction.
- `docs/product/mvp-definition.md` — the first household-administration loop and scope.
- `docs/product/competency-map.md` — the competency-led development map.
- `docs/architecture/agent-architecture.md` — the intended one-agent reasoning and execution boundary.
- `docs/domain-model.md` — domain boundaries, relationships and invariants.
- `docs/adr/` — accepted architectural decisions.
- `docs/plans/mvp-reset-follow-up-issues.md` — the post-reset implementation sequence.
- `docs/plans/unified-uploaded-document-household-work.md` — the first implementation-ready Household Work slice (GitHub issue #73).
- `docs/plans/pr61-extraction-map.md` — the PR #61 extraction and closure strategy.
- `docs/plans/luna-first-vertical-tickets.md` — the superseded historical ticket map.
- `docs/quality-gates.md` — approved test seams and QA/QC stop points.
- `frontend/app/prototype/luna/` — the throwaway Option A visual prototype.

## Desktop foundation

Production work lives in `desktop/`, a Tauri 2 application with a React interface and Rust local core.

```powershell
pnpm install --frozen-lockfile
pnpm --filter luna-desktop test
pnpm --filter luna-desktop typecheck
pnpm --filter luna-desktop tauri dev
```

The installed-application seam is built and run separately:

```powershell
pnpm --filter luna-desktop build:review
pnpm --filter luna-desktop build:e2e
pnpm --filter luna-desktop test:e2e
```

Use `build:review` before reviewing the Windows desktop application. It rebuilds the frontend through Tauri's `beforeBuildCommand` and then builds the no-bundle release executable at `desktop/src-tauri/target/release/luna-desktop.exe`. The review executable embeds the frontend assets, so it does not depend on a Vite server or `localhost:1420` being available.

Windows development requires Rust and the Visual Studio C++ build workload. macOS development requires Rust and Xcode command-line tools.

### Supporting document inspection

Luna may inspect documents locally for transport validation, preservation, evidence and recovery. The MVP uses Luna-managed OpenAI as the reasoning and document-reading engine when authorised context is required. Digital PDFs are read through the bundled Rust parser; image OCR uses local Tesseract. Image-only PDFs are first rasterised locally with Poppler's `pdftoppm`. Set `LUNA_TESSERACT_COMMAND` and `LUNA_PDFTOPPM_COMMAND` when those executables are not already available on the device path.

### Product reset

The repository contains an earlier document-centred vertical and its implementation history. That work is preserved for salvageable infrastructure and historical evidence, but it is not the next product roadmap. The MVP reset is recorded in `docs/product/`, `docs/architecture/agent-architecture.md` and `docs/plans/mvp-reset-assessment.md`.

### Local account service

Luna's beta account boundary uses Supabase Auth and PostgreSQL. Docker Desktop must be running for local account development.

```powershell
pnpm --filter luna-desktop supabase:start
pnpm --filter luna-desktop supabase:lint
pnpm --filter luna-desktop test:account-contract
```

Copy `desktop/.env.example` to `desktop/.env.local`, then use the local `API_URL` and `PUBLISHABLE_KEY` reported by `pnpm --filter luna-desktop exec supabase status --output json --workdir ..`. The publishable key is intended for the desktop client; secret and service-role keys must never be added to a Vite environment file.

Stop the local account service with `pnpm --filter luna-desktop supabase:stop`.

The abandoned pre-clean-sheet implementation is preserved under `scrapped-designs/pre-clean-sheet-luna/` and is not the foundation for new production code.
