# Luna

Luna is a digital household employee that takes ownership of household administration. It observes incoming household information, understands it in context, creates durable Household Work, proposes or completes authorised actions, and keeps the household involved only when genuinely necessary.

The MVP is web-first and briefing-led. Members open `Today` to see what Luna handled, what needs attention and what is upcoming, then delegate in ordinary language through a persistent conversation. Typed work state remains behind the interaction; structured inspection and correction are available without turning the main experience into a form.

## Current design sources

- `CONTEXT.md` — the domain glossary and preferred language.
- `docs/product/product-constitution.md` — binding product direction.
- `docs/product/mvp-definition.md` — the first household-administration loop and scope.
- `docs/product/competency-map.md` — competency-led development map.
- `docs/product/web-mvp-experience.md` — governing web interaction and responsive behavior.
- `docs/architecture/agent-architecture.md` — the one-agent reasoning and execution boundary.
- `docs/domain-model.md` — domain boundaries, relationships and invariants.
- `docs/adr/0020-make-the-luna-mvp-web-first.md` — accepted web-first and desktop-deferral decision.
- `docs/adr/0021-use-a-minimal-rust-web-adapter-for-household-administration.md` — accepted web-to-engine adapter and persistence decision.
- `docs/architecture/web-household-administration-backend.md` — HTTP, persistence and source boundaries.
- `docs/plans/web-first-migration-assessment.md` — desktop freeze, reusable work and web-service migration boundaries.
- `docs/plans/unified-uploaded-document-household-work.md` — the first Household Work capability slice.
- `docs/plans/luna-first-vertical-tickets.md` — superseded historical desktop ticket map.
- `docs/quality-gates.md` — approved test seams and QA/QC stop points.

## Web application

The production frontend lives in `apps/web`. Its primary route is `/today`, promoted from approved prototype Variant A. Production uses the HTTP `TodayService` adapter and the Rust service in `services/web-api`; the mock adapter remains for isolated frontend tests.

```powershell
pnpm install --frozen-lockfile
$env:OPENAI_API_KEY="<LUNA_PROJECT_API_KEY>"
$env:LUNA_OPENAI_MODEL="<EXPLICIT_SUPPORTED_MODEL>"
cargo run --manifest-path services/web-api/Cargo.toml
```

In a second terminal:

```powershell
pnpm --filter luna-web-prototype dev
```

Open <http://localhost:3000/today>. The root route redirects there. The historical founder-review prototype remains at <http://localhost:3000/prototype/web-first> for explicit design comparison only; it is not part of production navigation.

```powershell
pnpm --filter luna-web-prototype test
pnpm --filter luna-web-prototype typecheck
pnpm --filter luna-web-prototype build
```

The backend binds only to `127.0.0.1:8787` and stores local development state under `.luna/web` by default. `LUNA_WEB_DATA_DIR` changes that private location. Optional `LUNA_DEV_MEMBER_ID`, `LUNA_DEV_MEMBER_NAME`, `LUNA_DEV_HOUSEHOLD_ID` and `LUNA_DEV_HOUSEHOLD_NAME` values configure the trusted local review identity. These are development defaults, not production authentication.

The production route includes the proactive briefing, durable Household Work, one global conversation, bounded PDF/JPG/PNG upload, explicit approval/correction/completion/dismissal and refresh persistence. Gmail, cloud storage, workers and Daily Briefing automation remain deferred.

## Deferred desktop application

The Tauri 2 desktop application remains under `desktop/` as deferred capability and reusable domain evidence. Do not continue desktop UI, local filesystem Cabinet, portable-memory UX, native integration or local-model product work for the MVP. Desktop security-preservation work and explicitly approved maintenance still use the existing desktop commands and quality gates.

The historical `frontend/app/prototype/luna/` Option A experiment is preserved as design evidence. Its document-centred hierarchy is not the web MVP foundation.

### Historical supporting document inspection

The deferred desktop code may inspect documents locally for transport validation, preservation, evidence and recovery. Local OCR is not the primary web MVP path. The web prototype performs no document processing or upload.

### Product resets

The repository contains an earlier document-centred desktop vertical and its implementation history. That work is preserved for reusable contracts and historical evidence, but it is not the next product roadmap. The Household Work reset is recorded by ADR 0019. The web-first reset is recorded by ADR 0020, `docs/product/web-mvp-experience.md` and `docs/plans/web-first-migration-assessment.md`.

### Existing account service

Luna's beta account boundary uses Supabase Auth and PostgreSQL. It remains a candidate foundation for the future web identity boundary, but the mock prototype does not use it. Existing desktop account-contract work can still be run with:

```powershell
pnpm --filter luna-desktop supabase:start
pnpm --filter luna-desktop supabase:lint
pnpm --filter luna-desktop test:account-contract
```

Do not place secret or service-role keys in a browser environment file. The future web authentication and household-authorisation path requires a separately reviewed service contract.

The abandoned pre-clean-sheet implementation is preserved under `scrapped-designs/pre-clean-sheet-luna/` and is not the foundation for new production code.
