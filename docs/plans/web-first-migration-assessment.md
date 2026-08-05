# Web-first migration assessment

**Status:** Approved frontend reset assessment

## Executive assessment

Luna should not port the Tauri interface to the browser. The web MVP needs a new briefing-led interface and a multi-user service boundary. Existing desktop and Rust work is valuable when it captures Household Work, authority, validation, source, audit or recovery rules; it is not automatically reusable when it assumes a local process, local SQLite database, filesystem path, operating-system vault or Trusted Device.

The first production frontend foundation is the fixture-backed `/today` route in `apps/web`, promoted from accepted prototype Variant A. It intentionally does not connect a production backend, OpenAI, authentication, email or storage providers.

## Existing-work classification

### Reusable backend and domain logic

| Existing area | Reuse direction |
| --- | --- |
| `HouseholdAdministrationEngine::handle_turn` and its narrow ports | Adapt the callable engine behind a server-owned web host; preserve stable IDs, one-work-per-source behavior and terminal/no-op invariants. |
| Household request/result contracts and validation in `desktop/src-tauri/src/household_administration/` | Reuse schemas, context categories, evidence references and Luna-owned validation without importing Tauri or local persistence. |
| Direct Responses transport and bounded PDF/image handling in `household_administration/openai.rs` | Keep credentials and source transport server-side. LiteLLM remains deferred historical compatibility code. |
| Audit, idempotency and deterministic reasoning seams | Reuse concepts and tests; redesign storage and concurrency for a shared service. |
| Supabase account/household contracts | Evaluate as the initial web identity and relational persistence foundation; enforce household membership and authority server-side. |
| Source and conversation terminology | Retain: sources provide evidence, Conversation is the interaction, Household Work owns lifecycle. |

Rust modules can become a web service, a library behind an API or a source for ported rules. The decision depends on interface depth and service fit, not language loyalty.

### Reusable tests and contracts

- untrusted structured-output acceptance and rejection;
- envelope-owned metadata;
- bounded supported document input;
- `operation: none` as a true no-op;
- Luna-owned terminal transitions;
- correction, dismissal and same-work updates;
- terminal work excluded from attention;
- source linkage, audit and recovery semantics; and
- deterministic fixtures that do not require paid provider calls.

Installed desktop UI tests are historical evidence. They should not define the web interaction.

### Desktop-specific infrastructure — frozen

- Tauri commands, invoke adapters and native file pickers;
- local SQLite connection ownership and process-local transactions;
- operating-system credential vault and device provisioning UX;
- local filesystem Cabinet paths, presets, staging and file monitoring;
- portable-memory delivery through Cabinet files;
- Device PIN, Recovery Key and Trusted Device onboarding;
- local OCR and parser packaging as the primary document path;
- desktop packaging, updater and native WebdriverIO harness; and
- local/offline model distribution.

These systems remain in the repository and may receive security-preservation maintenance. They are not dependencies of the web MVP.

### Historical prototype UI

The Tauri React interface and `frontend/app/prototype/luna` are historical prototypes. Do not carry forward:

- document review forms and field-by-field clarification;
- To-do as a separate document-owned lifecycle;
- Cabinet path selection as onboarding or first value;
- provider/model configuration in the primary experience;
- offline/trusted-device status as the main shell hierarchy;
- document preview as the centre of the workspace;
- generic chat with an empty opening state; or
- desktop three-column layouts forced onto narrow screens.

The web prototype may reuse visual lessons such as a persistent composer and explicit context, but not the old information architecture.

## Merged Household Administration foundation

PR #76 was accepted and squash-merged as `5ca15c0f03978472fce9b01fccf667b53c63498a`. It established the reusable, platform-independent Household Administration engine, direct OpenAI adapter, strict structured output, bounded document transport, no-op behavior, terminal authority, attention projection and source-linked updates. Its headless live clarification, correction and scanned-image diagnostics passed before merge.

Do not connect the merged desktop-hosted implementation directly to browser modules. After the production `/today` route receives founder acceptance and the web service contract is defined:

1. map the accepted web-facing view and command contracts to the smallest authenticated service interface;
2. adapt reviewed engine ports, IDs, persistence and credentials to the multi-user boundary;
3. preserve Luna-owned authority, validation, audit, idempotency and bounded source rules;
4. expose a versioned interface consumed by `apps/web`; and
5. replace one mocked uploaded-document journey before adding connectors.

The mock prototype must remain decoupled until that contract is stable.

## Multi-user web redesign requirements

| Desktop assumption | Web requirement |
| --- | --- |
| One local process owns state | Authorised service owns shared household state across sessions and members. |
| SQLite provides serial local writes | Relational persistence provides optimistic concurrency, transactions, idempotency and household scoping. |
| Tauri command caller is the installed client | Every API request authenticates the member and authorises the household, target work and action. |
| OS vault stores device/provider credentials | Browser stores only web session material; provider and connector credentials remain server-side. |
| Filesystem paths identify documents | Stable logical source IDs reference user-controlled provider objects and versions. |
| Device encryption controls shared memory | Service privacy, row/object authorization, encryption and audit require an explicit web threat model. |
| Local event loop performs retries | Durable jobs will eventually own bounded retries and monitoring; background workers are not part of this prototype. |
| One device avoids simultaneous edits | Commands require version checks, idempotency keys and auditable conflict behavior. |

Production authentication may reuse Supabase only after verifying cookie/session handling, server-side authorization and household membership. The prototype does not implement it.

## Cabinet migration

The Cabinet remains a user-facing logical layer for household sources and records. It no longer means a local directory tree.

Future architecture:

```text
Cabinet item (Luna meaning, household relationships, source reference)
        |
        v
Storage-provider adapter (later)
        |
        v
User-controlled provider object (bytes, versions, sharing, retention)
```

Luna does not build managed file storage in the MVP. Google Drive, OneDrive and Dropbox adapters, OAuth and background synchronization are explicitly separate tasks after the uploaded-document web journey is proven.

## Repository layout

The least disruptive current layout is:

```text
apps/
  web/          # production Today frontend with a mock service adapter
desktop/        # deferred Tauri application; unchanged
frontend/       # historical Option A prototype
```

The intended service layout remains:

```text
services/
  api/
  household-work/
  intelligence/
  conversation/
  connectors/
```

Do not create empty service packages or move large code areas until the first API boundary is designed. Connectors remain deferred.

## Conflict record

The following prior directions conflict with ADR 0020 and are historical or must be updated:

- desktop production foundation as the primary application;
- local filesystem Cabinet as required household storage;
- connected email as the first MVP source;
- installed-app validation as the universal UI quality gate;
- portable memory and Trusted Device UX as MVP prerequisites; and
- local OCR as the primary document-understanding path.

Household Work, Conversation, OpenAI reasoning, Luna-owned authority, source evidence, audit and recovery remain governing directions.

## Recommended migration sequence

1. **Complete:** founder selected Variant A as the interaction hierarchy.
2. **In founder review:** Variant A is rewritten as the production `/today` route behind a mock service adapter.
3. Define the minimal authenticated web API for briefing, conversation, Household Work commands and bounded upload only after route acceptance.
4. Adapt the merged reusable schemas, rules and tests behind that API without importing desktop runtime dependencies.
5. Implement one persisted uploaded-document Household Work journey in the web app.
6. Add production authentication, household authorization, audit and concurrency evidence.
7. Prove the journey before designing background briefing generation, email or cloud-storage connectors.
