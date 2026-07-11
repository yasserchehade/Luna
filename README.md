# Luna

Luna is being rebuilt from a clean sheet as a permissioned AI household employee that learns how a household handles administrative records while leaving people in control of privacy, authority and consequential decisions.

## Current design sources

- `CONTEXT.md` — the domain glossary and preferred language.
- `docs/domain-model.md` — domain boundaries, relationships and invariants.
- `docs/adr/` — accepted architectural decisions.
- `docs/plans/luna-first-vertical-tickets.md` — the dependency-aware implementation path.
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
pnpm --filter luna-desktop build:e2e
pnpm --filter luna-desktop test:e2e
```

Windows development requires Rust and the Visual Studio C++ build workload. macOS development requires Rust and Xcode command-line tools.

The abandoned pre-clean-sheet implementation is preserved under `scrapped-designs/pre-clean-sheet-luna/` and is not the foundation for new production code.
