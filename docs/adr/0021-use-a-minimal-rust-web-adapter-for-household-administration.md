# ADR 0021: Use a minimal Rust web adapter for Household Administration

**Status:** Accepted

## Context

The accepted `Today` route was initially backed by an in-browser mock. The uploaded-document MVP now needs durable conversation, Household Work, source metadata and audit history while preserving the merged Rust `HouseholdAdministrationEngine`. Reimplementing interpretation or lifecycle rules in TypeScript would create two authorities and weaken Luna-owned validation.

## Decision

Use a narrow Axum service as the web adapter around the existing Rust engine. Production `TodayService` calls this service through same-origin Next.js rewrites. Direct OpenAI remains server-side behind `HouseholdAdministrationReasoning` and `household-administration.v1`.

Use SQLite plus a private bounded source directory as the first persistence adapter. Expose opaque source identifiers, never filesystem paths. Keep one global household conversation. Wrap each engine turn or explicit Luna-owned command in one database transaction.

Explicit approval, completion, dismissal and fact-correction buttons call a small engine command entry point. OpenAI is not asked to authorise those transitions. The mock `TodayService` remains available only for isolated frontend tests and historical prototype behavior.

## Consequences

The approved route can now use the same Household Administration behavior as headless diagnostics without Tauri, desktop unlock or Device PIN. Refresh reconstructs Today from durable state. The browser contains no OpenAI credential or duplicated domain logic.

This adapter is intentionally local-development infrastructure. Production identity, deployment, multi-instance storage, background intake and connected sources require later decisions. They do not justify a generic API, workflow system or provider abstraction in this milestone.
