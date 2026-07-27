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
pnpm --filter luna-desktop build:review
pnpm --filter luna-desktop build:e2e
pnpm --filter luna-desktop test:e2e
```

Use `build:review` before reviewing the Windows desktop application. It rebuilds the frontend through Tauri's `beforeBuildCommand` and then rebuilds the debug executable at `desktop/src-tauri/target/debug/luna-desktop.exe`.

Windows development requires Rust and the Visual Studio C++ build workload. macOS development requires Rust and Xcode command-line tools.

### Local document inspection

Luna keeps document inspection on-device. Digital PDFs are read through the bundled Rust parser; image OCR uses local Tesseract. Image-only PDFs are first rasterised locally with Poppler's `pdftoppm`. Set `LUNA_TESSERACT_COMMAND` and `LUNA_PDFTOPPM_COMMAND` when those executables are not already available on the device path.

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
