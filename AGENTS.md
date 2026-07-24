## Agent skills

### Issue tracker

Specifications and implementation tickets are tracked in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the five standard Matt Pocock workflow labels. See `docs/agents/triage-labels.md`.

### Domain docs

Luna uses a single-context domain model. See `docs/agents/domain.md`.

### Project documentation

When a change affects user-facing behaviour, architecture, delivery status, workflow, or quality evidence, update both the Obsidian project wiki and the GitHub wiki when needed. Keep local-only implementation status distinct from work published to GitHub.

### Local review application

After any change that affects runtime behaviour, the interface, configuration, or packaging, rebuild the local review application with `pnpm --filter luna-desktop build:review` before handing the work back to the user. Confirm that the build succeeds and report the absolute executable path and source commit so the user can inspect the result. A local review build is inspection evidence only and does not replace required CI or signed release artifacts.
