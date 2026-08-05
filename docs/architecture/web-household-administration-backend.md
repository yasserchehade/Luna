# Web Household Administration backend

**Status:** Implemented MVP boundary

The production `/today` route uses a minimal Rust HTTP service in `services/web-api`. The browser calls the typed `TodayService` HTTP adapter; the service derives household and member identity from trusted server configuration and calls the existing `HouseholdAdministrationEngine`. The browser never receives the OpenAI key and does not construct provider requests.

```text
Today route -> HTTP TodayService -> web API -> HouseholdAdministrationEngine
                                           -> direct OpenAI Responses adapter
                                           -> SQLite and bounded source store
```

## Boundary ownership

The engine continues to own conversation context assembly, OpenAI request construction, strict `household-administration.v1` response validation, Household Work interpretation, correction and terminal-transition validation. The new `handle_work_command` entry point owns explicit member commands for approval, completion, dismissal and fact correction; these commands do not grant OpenAI authority.

The web service owns HTTP translation, trusted development identity, transaction boundaries, SQLite persistence, opaque source references and Today projections. The React application owns presentation and optimistic interaction only. It does not copy Household Administration logic.

Every engine turn or explicit command runs inside one SQLite `BEGIN IMMEDIATE` transaction. Conversation messages, Household Work and audit records commit together. A reasoning, validation or persistence failure rolls the turn back, including the member message at the durable boundary.

## Minimal API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/today` | Load the global conversation and open Household Work projection |
| `GET` | `/api/household-work/{id}` | Load one work item, including its durable audit history |
| `POST` | `/api/conversation` | Run one global conversation turn, optionally with an opaque source reference |
| `POST` | `/api/sources` | Accept one bounded PDF, JPG or PNG source |
| `POST` | `/api/household-work/{id}/approve/{actionId}` | Record explicit member approval in Luna-owned logic |
| `POST` | `/api/household-work/{id}/dismiss` | Record explicit dismissal |
| `POST` | `/api/household-work/{id}/complete` | Record explicit completion |
| `POST` | `/api/household-work/{id}/facts` | Correct one existing fact while preserving other state |

This is not a generic workflow API. It exposes only what the accepted `Today` experience needs.

## Persistence and sources

The MVP adapter stores conversation messages, full Household Work payloads, source metadata and audit records in `luna-web.sqlite3`. Uploaded bytes are stored below the configured private data directory under random opaque storage keys. API responses expose only `source-*` identifiers, safe display names, media types and sizes; local filesystem paths and storage keys never cross the HTTP boundary.

The existing 5 MiB source limit applies before persistence and again before provider request construction. Accepted media types are PDF, JPEG and PNG. The engine sends bounded source bytes through the supported OpenAI file/image representation and retains its existing 12,000-character extracted-text bound. See [ADR 0019](../adr/0019-openai-mvp-household-administration-agent.md).

## Current limitations

- Identity is trusted local-development configuration, not production authentication or household authorisation.
- SQLite and the local source store are a single-process MVP adapter, not a deployed multi-instance store.
- Upload processing is synchronous with the member turn; background review is deferred.
- The desktop crate remains the physical package containing the extracted Rust library. The backend calls its platform-independent exports, but a later packaging-only change may move those modules into a dedicated core crate.
- Gmail, cloud storage, workers, notifications, Daily Briefing automation and external action execution remain out of scope.
