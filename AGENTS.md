## Agent skills

### Issue tracker

Specifications and implementation tickets are tracked in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the five standard Matt Pocock workflow labels. See `docs/agents/triage-labels.md`.

### Domain docs

Luna uses a single-context domain model. See `docs/agents/domain.md`.

### Product-direction gate

The binding product direction is in `docs/product/product-constitution.md`, `docs/product/mvp-definition.md`, `docs/product/competency-map.md`, `docs/product/web-mvp-experience.md`, `docs/architecture/agent-architecture.md` and ADR 0020.

Before planning or implementing a task, Codex must evaluate:

- Does this move Luna toward being a digital household employee?
- Does it reduce household administrative work?
- Does it minimise member effort?
- Does it preserve natural conversation?
- Does it avoid asking for known information?
- Does it support the MVP?
- Is the complexity needed now?
- Does it make household work, rather than documents or UI forms, the durable centre?
- Does it incorrectly restrict OpenAI from receiving context required for useful reasoning?
- Is it adding infrastructure for hypothetical future requirements?
- Does it preserve the web-first `Today` briefing and persistent conversation rather than recreating the desktop UI?

Codex must stop and identify a product conflict before implementing work that violates these principles. Existing document-first, desktop-first and provider-neutral/local-first decisions are historical where the reset assessments or ADRs 0019 and 0020 mark them superseded. Tauri UI development, local filesystem Cabinet product work and desktop-only UX are frozen for the MVP.

### Project documentation

When a change affects user-facing behaviour, architecture, delivery status, workflow, or quality evidence, update both the Obsidian project wiki and the GitHub wiki when needed. Keep local-only implementation status distinct from work published to GitHub.

### Local review applications

After a change to the web prototype or future web interface, run its tests, typecheck and `pnpm --filter luna-web-prototype build`, then provide the local review URL and source commit. After an explicitly approved change to deferred desktop runtime behaviour, interface, configuration or packaging, rebuild with `pnpm --filter luna-desktop build:review` and report the absolute executable path and source commit. Do not rebuild the desktop merely because web or documentation files changed. Review builds are inspection evidence only and do not replace required CI or release artifacts.
