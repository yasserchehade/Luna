# Luna Agent Architecture

**Status:** Intended MVP architecture

This document defines the conceptual architecture for the household-administration agent. It replaces the assumption that ordinary chat and document interpretation are separate product paths.

## One household-administration reasoning loop

```text
User message, email or document
        |
        v
Relevant household context assembly
        |
        v
OpenAI household-administration reasoning
        |
        v
Conversational response or proposed tool calls
        |
        v
Luna validation and approval boundary
        |
        v
Tool execution
        |
        v
Durable household work and audit update
        |
        v
Natural explanation to the user
```

The loop may pause for missing information, approval, connectivity or a tool result. A pause is a state of the same household work, not a hand-off to an unrelated document workflow.

## Callable engine seam

The first implementation seam for this loop is `HouseholdAdministrationEngine::handle_turn` in `desktop/src-tauri/src/household_administration/`. It accepts platform-independent household, conversation, actor, message, source/work reference, authorised context, action-definition and correlation values. It returns a natural response, resulting Household Work, clarification/action proposals, audit events and a typed safe failure category.

The engine uses narrow Rust interfaces for:

- recent conversation reads and member/Luna message appends;
- Household Work listing and persistence;
- bounded logical source retrieval;
- untrusted Household Administration reasoning; and
- deterministic time.

The same interface is exercised by in-memory integration adapters and the Tauri compatibility adapters. A managed reasoning adapter can receive an explicitly injected endpoint and narrow gateway credential for opt-in headless live diagnostics, so the engine does not need a Device PIN or operating-system vault.

Tauri remains responsible for the desktop session, legacy `DocumentArrival` translation, local file access, encrypted SQLite, OS-vault credentials and desktop compatibility capture. A future web host should supply server-side adapters to this same engine interface; this decision does not introduce a web endpoint, web authentication, storage connector or background worker.

## Conceptual responsibilities

### Sources

Email, attachments, user messages, documents, calendar events and future message services are sources of information. They contribute evidence and references. A source does not own the household work created from it.

### Relevant household context assembly

Before reasoning, Luna assembles the smallest authorised context that is sufficient to understand the current situation. For the MVP this includes, when relevant:

- the current user message;
- recent relevant conversation, not only the newest sentence;
- the email body, sender, recipients, subject and received time;
- the complete supported attachment needed to read the source;
- active household work and related source history;
- confirmed household members, properties, accounts, providers and responsibilities;
- prior member decisions and standing directions; and
- authority, privacy and execution constraints.

Context selection is a Luna-owned policy. It must be transparent enough to explain what was used, and it must not omit context merely to preserve an artificial single-message provider contract.

### OpenAI household-administration reasoning

The MVP uses OpenAI as the reasoning and document-reading engine. OpenAI may:

- read authorised email and attachments;
- interpret facts and relationships;
- identify whether work needs attention;
- summarise and explain;
- ask a clarifying question;
- propose a durable work update; and
- propose a tool call.

OpenAI does not own Luna's household memory, authentication, permissions, authority, durable state, tool implementation, execution, audit record or recovery. Provider output is untrusted data until Luna validates it.

### Conversational response and proposed tools

The reasoning result should express a natural response plus typed proposals. A proposal may include creating or updating household work, drafting a reply, creating a reminder, requesting approval or calling a named tool. The model does not directly invoke the tool or write durable state.

The user should see the outcome and the next meaningful choice. Internal routing, schemas and state-machine mechanics remain behind the interaction except where transparency or approval requires them.

### Luna validation and approval boundary

Luna validates:

- request correlation and source identity;
- allowed response fields and value constraints;
- the relationship between evidence and proposed facts;
- authority, target and scope;
- duplicate or conflicting work updates;
- approval requirements and standing directions;
- tool arguments and idempotency; and
- persistence and audit requirements.

Luna rejects malformed, ambiguous, over-scoped or unauthorised proposals. It never treats model confidence as authority.

### Tool execution

Luna owns authentication, permission checks, validation, execution, retries, idempotency, error mapping and recovery for every tool. Tools may create drafts, schedule reminders, send approved messages or call a connected service. Tool results return to the same household work item and reasoning context.

### Durable household work and audit

Household work is the central durable domain. An internal work item may be called `Obligation` when a stable technical term is needed. It owns the lifecycle from observed information through understanding, member input, approval, execution, monitoring, completion, dismissal or irrelevance.

Email and documents remain durable sources and evidence. Conversation remains the interaction layer and may reference work. Tools remain the execution layer. None of these may become a competing durable owner.

## One reasoning layer

User messages, emails and documents enter the same context assembly and reasoning process. The architecture must not split into:

- an ordinary-chat path that sees only the newest sentence; and
- a deterministic document questionnaire that owns extraction and asks the member to complete fields.

Deterministic parsers, OCR, schemas and review surfaces remain useful supporting mechanisms for transport, evidence, validation, correction and recovery. They must not intercept the primary conversation or require a member to repeat facts that the email or attachment already contains.

## Privacy and authorised context

Privacy is intentional context control, not context starvation. Luna should disclose only authorised, relevant information, but the MVP must be able to send OpenAI the conversation, attachment and household situation needed to reason correctly. A single-message request that cannot resolve "this bill," "the property we discussed" or "the reminder from last week" is not a successful privacy boundary.

Luna records the source references, context policy, provider route, model result summary, proposed action, approval, execution and recovery outcome without placing secrets or unbounded hidden reasoning in durable household work.

## Failure and recovery

If OpenAI, a source or a tool is unavailable:

- save the source and the household work state;
- explain what is waiting and why;
- preserve the exact request or source needed for a safe retry where policy allows;
- retry only within Luna-owned bounded, idempotent rules;
- never silently switch provider, target or action scope; and
- resume the same work item without creating duplicates.

An invalid model proposal is a failed interpretation, not a member approval and not a partial execution.

## MVP implementation boundary

The existing Luna-managed OpenAI gateway, secure credential vault, attachment transport, persistence, audit and deterministic test seams can support this architecture. The current split between `ConversationReply` and `DirectionInterpretation`, the latest-message-only disclosure contract and the document-led `Document Handling` lifecycle are migration targets, not the intended product boundary. See [the MVP reset assessment](../plans/mvp-reset-assessment.md).
