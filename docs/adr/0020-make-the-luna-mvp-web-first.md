---
status: accepted
date: 2026-08-04
---

# Make the Luna MVP web-first and defer the desktop application

## Context

Luna began as a Tauri desktop application with a React interface, a Rust local core, local SQLite persistence, a user-owned filesystem Cabinet, device-bound encryption, portable memory and local document inspection. That work proved useful authority, recovery, document and Household Work boundaries, but it also coupled the product experience to one installed device and made local storage and native capabilities feel like prerequisites for household-administration value.

The product reset in ADR 0019 established Luna as a digital household employee, with Conversation as the primary interaction and Household Work as the durable centre. Founder review then showed that the current desktop interface still behaves too much like document handling with conversation layered over it. PR #76 improves the shared Household Work reasoning path but retains live clarification, correction and image-delegation failures and is not approved for merge.

The MVP needs the fastest path to validating whether families value a proactive household employee. Native filesystem access, local models, offline operation and operating-system integration are not required to answer that product question. A responsive web application makes the core experience easier to reach, review, iterate and use across household members and devices.

## Decision

The Luna MVP is web-first.

Luna will build a responsive web application as its primary MVP surface. `Today` is the home view: Luna starts with a concise briefing of completed work, matters requiring attention or approval and upcoming obligations. Conversation remains available throughout through a persistent composer. Household Work appears as structured reports inside the briefing and conversation rather than as a dashboard, task manager or form workflow. A contextual panel shows only the source, household entity, understood facts, unresolved need and proposed action relevant to the selected work.

The desktop client is deferred, not deleted. Existing desktop code remains available as evidence, reusable domain logic, reusable contracts and deferred native capability. Tauri UI development and desktop-only product work are frozen for the MVP.

The Cabinet becomes a logical household-document layer. The MVP will not depend on a local filesystem Cabinet. When storage connectivity is implemented in a later task, the Cabinet will use connected user-controlled cloud storage rather than Luna-managed file storage.

Luna owns document meaning, relationships and Household Work. Storage providers own file bytes, versions, sharing and storage infrastructure.

Luna should use existing infrastructure when that infrastructure is not part of Luna's differentiation. Authentication, relational persistence, object-provider storage, delivery and observability should be bought or reused where practical. Luna's differentiated work is authorised context assembly, household reasoning, safe authority and execution, durable Household Work and natural member interaction.

ADR 0019 remains authoritative for the OpenAI reasoning and Luna-owned execution boundary. This ADR changes the primary product surface, persistence topology and document-storage direction; it does not make model output trusted or redesign Household Work.

## Frozen desktop scope

The following are frozen for the MVP and must not be extended without a new accepted decision:

- Tauri UI development and desktop navigation redesign;
- local filesystem Cabinet features and native file monitoring;
- desktop-only settings, onboarding and portable-memory UX;
- local OCR as the primary product path;
- native operating-system integrations;
- local-model product work; and
- desktop packaging work except security, preservation or explicitly approved maintenance.

These systems remain in the repository. They are not production requirements for the web MVP.

## Reusable work

The web implementation may reuse or port:

- Household Work types, lifecycle rules, terminal-state authority and no-op invariants;
- OpenAI request/result schemas, bounded source contracts and untrusted-result validation;
- conversation, source-reference, audit, idempotency and recovery concepts;
- account and household identity contracts that are valid in a multi-user service;
- deterministic tests and synthetic fixtures; and
- managed-intelligence gateway boundaries where they are not coupled to a Trusted Device or desktop credential vault.

Rust is an implementation option, not a preservation requirement. Domain rules may be exposed behind a web API, ported or replaced when local SQLite, filesystem paths, synchronous Tauri commands or device-bound encryption make the existing module unsuitable for a multi-user service.

PR #76 must not be merged as part of this decision. Its reusable contracts and domain fixes should be extracted only after its live-boundary failures are understood and after a stable web backend contract is defined.

## Migration consequences

- A new `apps/web` surface is created without moving or deleting the desktop application.
- The historical `frontend` prototype remains evidence and is not the web MVP foundation.
- Durable shared state moves from per-device SQLite toward an authorised server-side household service.
- Tauri command calls become versioned web API calls with household membership, concurrency, idempotency and audit enforced server-side.
- Filesystem paths and Cabinet destinations become provider-neutral logical source references. Future provider adapters map those references to user-controlled storage.
- Device PIN, recovery-key and portable-memory flows remain deferred desktop capabilities unless a separate threat-model decision proves they are needed for the web service.
- Web sessions cannot receive upstream OpenAI or storage-provider credentials. Credentials and tool execution remain server-side.
- Proactive briefing data is fixture-only in the first prototype. Background workers and Daily Briefing generation remain separate future work.

## Risks

| Risk | Response |
| --- | --- |
| Web delivery weakens household privacy | Enforce household membership, server-side authority, minimal authorised context and auditable disclosure; never expose provider credentials to the browser. |
| The web UI becomes another generic chatbot or dashboard | Keep Today briefing-led, Conversation persistent and Household Work outcome-oriented; reject KPI grids, inbox replicas and workflow configuration. |
| Existing desktop investment biases the service design | Classify code by reusable rule or desktop coupling; do not preserve modules solely because they already exist. |
| Cloud storage integration expands the MVP | Keep mock/local upload in the prototype and defer provider OAuth and adapters to a separately approved task. |
| A web rewrite bypasses proven safety rules | Port and test Luna-owned validation, authority, no-op, terminal transition, audit and recovery invariants before connecting live intelligence. |
| Proactive language overstates real automation | Clearly label fixture data in prototypes and do not claim background work until the service can evidence it. |

## Reconsidering a desktop application

A desktop client may be reconsidered only after the web MVP demonstrates demand and at least one of these needs is evidenced:

- native operating-system integration materially reduces household work;
- reliable local filesystem access or file monitoring is a validated user need;
- offline operation is necessary for a meaningful target segment;
- local-model execution meets the same reasoning and safety contract and provides material value; or
- desktop distribution provides a security or workflow benefit that cannot be achieved responsibly on the web.

Reconsideration requires a new ADR with product evidence, maintenance cost, cross-platform scope and a clear relationship to the web service. It must not fork the Household Work lifecycle.

## Superseded and retained records

- ADR 0005 and desktop-first roadmap commitments are historical for the MVP; their implementation evidence remains useful.
- ADRs 0001, 0002, 0004 and 0006–0009 describe deferred local Cabinet, device and portable-memory designs, not web MVP requirements.
- ADR 0010's conversation-first principle and ADR 0019's agent and authority boundaries remain accepted.
- Existing email-first MVP sequencing is superseded by the web-first uploaded-document prototype and service-contract sequence. Email and storage connectors follow only after the web Household Work journey is proven.
