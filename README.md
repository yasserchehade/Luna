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
- `docs/plans/web-first-migration-assessment.md` — desktop freeze, reusable work and web-service migration boundaries.
- `docs/plans/unified-uploaded-document-household-work.md` — the first Household Work capability slice.
- `docs/plans/luna-first-vertical-tickets.md` — superseded historical desktop ticket map.
- `docs/quality-gates.md` — approved test seams and QA/QC stop points.

## Web-first prototype

The founder-reviewable prototype lives in `apps/web`. It uses local fixture state and does not connect authentication, OpenAI, email, cloud storage or production services.

```powershell
pnpm install --frozen-lockfile
pnpm --filter luna-web-prototype dev
```

Open <http://localhost:3000/prototype/web-first?variant=A>. Use `variant=A`, `B` or `C` to compare the three information hierarchies. Loading, empty and error states are available with `state=loading`, `state=empty` and `state=error`.

```powershell
pnpm --filter luna-web-prototype test
pnpm --filter luna-web-prototype typecheck
pnpm --filter luna-web-prototype build
```

The prototype variants and switcher are throwaway design evidence. After founder selection, rewrite the selected hierarchy as the real `Today` route and remove the losing variants and switcher from production work.

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
