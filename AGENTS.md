## Agent skills

### Issue tracker

Specifications and implementation tickets are tracked in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the five standard Matt Pocock workflow labels. See `docs/agents/triage-labels.md`.

### Domain docs

Luna uses a single-context domain model. See `docs/agents/domain.md`.

### Product-direction gate

The binding product direction is in `docs/product/product-constitution.md`, `docs/product/mvp-definition.md`, `docs/product/competency-map.md` and `docs/architecture/agent-architecture.md`.

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

Codex must stop and identify a product conflict before implementing work that violates these principles. Existing document-first tickets and provider-neutral/local-first decisions are historical where the reset assessment or ADR 0019 marks them superseded.

### Project documentation

When a change affects user-facing behaviour, architecture, delivery status, workflow, or quality evidence, update both the Obsidian project wiki and the GitHub wiki when needed. Keep local-only implementation status distinct from work published to GitHub.

### Local review application

After any change that affects runtime behaviour, the interface, configuration, or packaging, rebuild the local review application with `pnpm --filter luna-desktop build:review` before handing the work back to the user. Confirm that the build succeeds and report the absolute executable path and source commit so the user can inspect the result. A local review build is inspection evidence only and does not replace required CI or signed release artifacts.
