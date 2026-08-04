---
status: accepted
date: 2026-08-03
---

# Use OpenAI as the MVP reasoning engine behind one household-administration agent

> **Delivery-surface note:** [ADR 0020](./0020-make-the-luna-mvp-web-first.md) makes the responsive web application the MVP surface and defers the desktop client. This ADR's reasoning, untrusted-output and Luna-owned authority boundaries remain accepted.

## Context

Luna's previous direction centred the first vertical on local-first Document Handling. Conversation was a primary interface over that document lifecycle, while a provider-neutral intelligence gateway and narrow consent grants protected a future choice among local models, BYOK and multiple cloud providers.

That direction produced useful foundations, but it also made the product behave like a document-management system with a chat surface. The current implementation has a separate ordinary-conversation route and document-intelligence route. Ordinary conversation sends only the newly submitted message, while document evaluation sends bounded fields and a document attachment. The model therefore cannot reliably use earlier conversation, household context or active work to understand references and decide what the household needs.

The MVP must prove that Luna reduces real family administration. It needs to understand an incoming email and attachment, create durable household work, discuss it naturally, prepare a safe action and monitor the result. Architectural completeness around local models, provider choice, per-document consent and billing is not required to prove that loop.

## Decision

For the MVP, Luna is one household-administration agent and OpenAI is its reasoning and document-reading engine.

Direct OpenAI is the MVP reasoning path. Luna calls the OpenAI Responses API from a trusted backend/runtime boundary using a server-side `OPENAI_API_KEY` and an explicitly configured `LUNA_OPENAI_MODEL`. LiteLLM, BYOK, provider-neutral routing and multi-provider support are deferred until justified by validated product demand. The internal `HouseholdAdministrationReasoning` port remains so this transport decision does not leak into Household Work or make model output trusted.

All relevant user messages, emails and documents enter one Luna-owned context assembly and reasoning loop. Luna provides OpenAI with authorised recent relevant conversation, household context, active work, email content and attachments when those are needed for useful reasoning. The model may read, interpret, summarise, ask a question, propose a durable work update or propose a tool call.

Household work is the central durable domain. An internal work item may be called `Obligation`; user-facing language should describe things needing attention, household work or what needs taking care of. Emails and documents are sources and evidence. Conversation is the primary interaction layer. Tools are the execution layer.

Luna remains the authority boundary. It owns authentication, context policy, permissions, validation, approval, tool execution, persistence, audit, idempotency, retries and recovery. OpenAI output is untrusted and cannot mutate household state, grant authority, execute an action or bypass approval.

The first useful action set is intentionally small: explain the work, prepare a draft reply or reminder, request approval and execute only a validated authorised action. Autonomous bill payment and other consequential financial actions are outside the MVP.

## What the previous direction got right

The following foundations are retained:

- conversation as the primary member interaction;
- Luna-owned household identity, authority and durable persistence;
- secure credential handling and trusted-device boundaries;
- preserved original attachments and source transport;
- schema and domain validation of untrusted provider output;
- audit and recovery for provider or tool failures; and
- deterministic seams for tests without paid provider calls.

The decision changes the product centre and reasoning contract, not the need for these controls.

## What remains controlled by Luna

Luna, not OpenAI, controls:

- which household context may be assembled and disclosed;
- identity, authentication and household membership;
- private/shared access and authority grants;
- the durable Household Work / `Obligation` lifecycle;
- source preservation and evidence provenance;
- response and tool schemas;
- approval and standing-direction checks;
- credentials and external service permissions;
- validation, idempotency and execution;
- audit history, monitoring and recovery; and
- what the member is told and what action is presented.

## Deferred decisions

The MVP defers local models, local-only reasoning, multiple intelligence providers, BYOK, provider-selection UX, complex per-document consent grants, generic workflow builders, autonomous bill payment, bank integrations, broad role and authority systems, advanced filing-rule learning and extensive billing infrastructure.

These remain possible future capabilities. They must not shape the MVP interaction or require the first household-administration loop to pass through provider configuration.

## Migration implications

1. Define durable Household Work / `Obligation` and make source records, conversations and views reference it.
2. Add the first email source and attachment ingestion path.
3. Build Luna-owned context assembly for recent relevant conversation, household context, active work, email and attachments.
4. Replace the latest-message-only conversation request and separate document questionnaire with one agent reasoning contract.
5. Use a direct server-side OpenAI Responses transport, supported PDF/image inputs, strict structured output, Luna-owned envelope metadata, validation and audit seams. Do not require a managed gateway for the MVP Household Administration path.
6. Move extraction and document review from the primary workflow into supporting evidence and correction surfaces.
7. Add Luna-owned proposals, approval and execution for a small reply/reminder tool set.
8. Add monitoring and end-to-end tests for active work through completion, dismissal and irrelevance.
9. Re-triage the old document-first tickets and preserve them as historical records rather than treating them as the next product roadmap.

## Consequences

Positive consequences:

- the MVP proves a household outcome rather than a field-extraction demo;
- the model can resolve references using the context required to understand them;
- email, documents and conversation share one reasoning layer;
- household work remains coherent across sources and follow-up actions; and
- the product can add sources and tools by strengthening competencies rather than creating separate experiences.

Costs and trade-offs:

- authorised household context must be assembled and transmitted deliberately;
- the MVP depends on a direct OpenAI API route, server-side key management and OpenAI usage cost;
- the current document-centred state model and tests require migration or replacement;
- some existing provider-neutral and consent infrastructure will be temporarily more capability than the MVP needs; and
- local-only operation cannot be promised for the first intelligence-dependent loop.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| OpenAI receives too much household information | Luna owns relevance, authority and disclosure policy; record the context categories used and keep secrets out of requests |
| The model proposes an unsafe action | Treat output as untrusted; validate target, scope, authority, approval and idempotency inside Luna |
| A cloud outage stops useful household work | Preserve sources and work, explain waiting, continue safe local bookkeeping and retry the same route within bounded rules |
| A single work item becomes ambiguous or duplicated | Use stable source references, explicit merge rules, lifecycle states and immutable audit events |
| Product scope expands into an integration platform | Require every feature to pass the constitution's north-star and MVP questions |

## Reconsidering local models or additional providers

Local models or other providers may be reconsidered after the MVP demonstrates the end-to-end loop and only if a candidate can meet the same Luna-owned contract. Evidence must include:

- equal or better understanding of email, attachments, conversation and household context;
- predictable structured proposals and safe failure semantics;
- privacy and retention controls that Luna can verify;
- acceptable latency, cost and operational burden;
- no weakening of Luna's authority, approval, audit or recovery boundary; and
- a migration path that does not reintroduce provider-selection work into the primary user experience.

## Superseded records

ADR 0003's product-level commitment to local-first, provider-neutral intelligence is superseded for the MVP by this decision. ADR 0015's server-side credential and untrusted-result boundaries remain useful, but its gateway, multi-provider and BYOK framing is deferred and is not the default MVP execution path. ADRs 0016 and 0017 remain historical records of deferred commercial/provider work, not MVP requirements.
