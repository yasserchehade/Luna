# Unified uploaded-document Household Work journey

**Status:** Implementation-ready first MVP slice; tracked in [GitHub issue #73](https://github.com/yasserchehade/Luna/issues/73)

**Dependencies:** Merge the documentation reset independently, then extract or port only the reusable PR #61 infrastructure. This slice must not begin Gmail integration and must not depend on PR #61 being merged.

## Issue statement

Prove the first unified Household Work agent journey with a user-uploaded household document:

> A user uploads a household document and communicates naturally. OpenAI receives the document, relevant conversation and household context, identifies Household Work, and Luna stores and discusses that work without asking the user to repeat information available in the document.

This is the first implementation slice for the agreed MVP. It is a local upload/source path used to prove the reasoning, durable work and conversation loop. Email ingestion is explicitly later.

## User journey

1. The user starts or continues a natural conversation.
2. The user uploads a household document.
3. The user may say something minimal such as, "Take care of this."
4. Luna assembles sufficient authorised context for OpenAI, including:
   - the uploaded document and preserved original bytes;
   - the current user message;
   - relevant recent conversation, not only the newest message;
   - relevant confirmed household context;
   - active related Household Work; and
   - the available Luna-owned tool definitions and execution constraints.
5. OpenAI interprets the document and user intent.
6. OpenAI proposes:
   - a natural response;
   - Household Work to create or update;
   - any genuinely necessary clarification; and
   - optional proposed next actions.
7. Luna validates and persists the result.
8. Luna responds naturally, explaining what it found, what needs attention and what it proposes.
9. The user may correct, dismiss or update the work conversationally.

The user must be able to ask a general question about the document without being forced into a document-field workflow.

## Required invariant: understand before asking

Luna must not ask the user for information already available in:

- the uploaded document;
- the current email or source, when a source wrapper is present;
- relevant conversation;
- household context;
- existing Household Work; or
- prior confirmed decisions.

The context assembler must record which context categories were supplied. A missing fact is a valid clarification only after those sources have been considered. The agent must not be restricted to only the newest user message.

## Minimum durable domain

Keep the first structure deliberately small. The internal name is `HouseholdWork`; `Obligation` is an acceptable domain alias.

```text
HouseholdWork
  id: stable identifier
  householdId: owning household
  status: active | needsClarification | awaitingApproval | inProgress |
          monitoring | completed | dismissed | noLongerRelevant | blocked
  kind: bill | renewal | request | appointment | other
  summary: concise member-facing description
  facts: list of WorkFact
  sourceRefs: uploaded source/document and conversation/message references
  responsibleMemberId: optional confirmed or proposed member reference
  dueAt: optional ISO timestamp/date
  urgency: optional low | normal | high
  proposedActions: list of persisted action proposals
  createdAt, updatedAt, closedAt: timestamps

WorkFact
  key: provider | property | account | amount | dueDate |
       requiredAction | urgency | other approved MVP fact
  value: bounded display value
  evidenceRefs: source/document/context references
  certainty: confirmed | likely | unknown

ProposedAction
  id: stable identifier
  kind: draftReply | reminder
  summary: natural-language description
  arguments: typed, allowlisted MVP arguments
  approval: notRequired | required | approved | declined
  execution: notStarted | completed | failed
```

The first slice does not create an exhaustive household ontology, generic workflow entities, hidden model memory or a second document-owned work lifecycle. The uploaded original remains a source/evidence record linked to `HouseholdWork`.

## Agent input contract

Luna owns the request envelope and supplies OpenAI with:

```text
HouseholdAdministrationRequest
  requestId
  conversationId
  currentMessage
  relevantConversation[]
  source: uploaded document metadata + original attachment
  householdContext: authorised relevant members/properties/accounts/providers/responsibilities
  activeHouseholdWork[]
  availableTools[]
  authorityAndApprovalConstraints
  responseSchemaVersion
```

`relevantConversation`, `householdContext`, `activeHouseholdWork` and `availableTools` are intentionally part of the first contract. They must not be reduced to a single `currentMessage` field.

## Minimum OpenAI output contract

The model returns one structured result. This is a bounded proposal schema, not a generic workflow language:

```text
HouseholdAdministrationResult
  reply: non-empty natural-language response
  work:
    operation: none | create | update
    workId: required for update, absent for create
    kind: optional allowlisted MVP kind
    summary: optional bounded summary
    status: optional proposed HouseholdWork status
    facts[]:
      key: allowlisted MVP fact key
      value: bounded value
      evidenceRefs[]: references into supplied source/context
      certainty: confirmed | likely | unknown
    dueAt: optional date/time supported by evidence
    urgency: optional low | normal | high
  clarification:
    question: optional one focused question
    reason: optional bounded explanation of what remains materially unknown
    field: optional allowlisted missing decision/context field
  proposedActions[]:
    kind: draftReply | reminder
    summary: bounded description
    arguments: typed allowlisted arguments
    approvalRequired: boolean proposal only
```

Rules:

- `reply` is always conversational and may explain that no Household Work was created.
- `work.operation = update` must reference an existing active work item supplied in context; the model cannot invent an identifier.
- Every proposed fact must cite a supplied source or context reference. A fact without evidence is omitted or marked unknown.
- At most one clarification is returned for the first slice.
- Proposed actions are suggestions. The model cannot approve, execute, mutate durable state or create arbitrary workflow steps.
- Unknown fields, extra actions, oversized values, malformed dates, unsupported references and conflicting work identifiers are rejected by Luna.

## Luna execution boundary

OpenAI may read, reason, summarise, ask one clarification and propose work or actions.

Luna owns:

- persistence of the source, conversation and Household Work;
- request correlation and evidence validation;
- status transitions and merge/update rules;
- authority and approval requirements;
- allowlisted tool definitions and argument validation;
- tool execution, idempotency and retries;
- audit events and source provenance;
- error mapping and blocked/waiting state; and
- the final natural explanation shown to the user.

No OpenAI response may directly call a tool, send a message, create a reminder, approve an action, close work or alter household context.

## Acceptance scenarios

### Electricity bill

The user uploads a bill containing the provider, property, amount and due date and says, "Take care of this."

Expected result:

- Luna extracts the provider, property, amount and due date;
- creates one Household Work item with those facts and evidence;
- explains what needs attention in natural language;
- does not ask the user to re-enter those facts; and
- proposes an appropriate next action, such as a reminder or draft response, with approval handled by Luna.

### Already completed

After the work is discussed, the user says that they already paid it.

Expected result:

- Luna identifies the referenced Household Work from relevant conversation and active work;
- updates the work to `completed` or records the appropriate completed outcome;
- preserves the source and audit history; and
- confirms the result naturally without asking for the bill details again.

### Ambiguous property

The document contains provider, amount and due date but does not clearly identify which of two known household properties it concerns.

Expected result:

- Luna asks one focused property clarification;
- it explains why the property matters to the work;
- it does not ask for unrelated fields already available in the document; and
- once answered, it updates the same Household Work item rather than creating a duplicate.

### General question

The user asks a question about the uploaded document rather than instructing Luna to process it, such as asking when the bill is due or what amount is shown.

Expected result:

- Luna answers naturally using the document and relevant context;
- it may mention a possible Household Work item when appropriate, but does not force creation if no work is requested or needed; and
- it does not route the message into a document-field prompt or form.

### Correction and dismissal

The user corrects a provider or says the document is irrelevant.

Expected result:

- Luna updates or dismisses the same Household Work item conversationally;
- it records the correction or dismissal as an auditable member decision; and
- it does not overwrite the preserved original.

## Explicit non-goals

This issue does not include:

- Gmail integration;
- Outlook integration;
- automatic payments;
- local models or local-only reasoning;
- multiple model providers;
- BYOK;
- generic workflows or a workflow language;
- complex role systems;
- an exhaustive household ontology;
- autonomous consequential actions;
- a document questionnaire under another name; or
- restricting OpenAI to only the newest user message.

## Smallest ordered replacement PR sequence

1. `codex/product-direction-reset` — documentation-only reset, based on `main`; merge independently.
2. `codex/pr61-infrastructure-extraction` — cherry-pick only product-neutral attachment, credential, recovery and managed OpenAI transport seams; do not merge PR #61.
3. `codex/household-work-foundation` — add the small `HouseholdWork` model, source references, status transitions, audit events and deterministic context-assembly contract.
4. `codex/household-work-openai-slice` — add the context-aware OpenAI request/result contract, Luna validation, persistence and managed route integration using deterministic tests first.
5. `codex/household-work-conversation` — connect upload and natural Conversation, support correction/dismissal/update, and add the installed-app acceptance scenarios.

The first slice is complete when the uploaded-document scenarios pass end to end. Gmail and other integrations remain follow-on competency work.

## Tracker relationships

- This issue supersedes the document-first implementation scope in issues [#1](https://github.com/yasserchehade/Luna/issues/1) and [#39](https://github.com/yasserchehade/Luna/issues/39) for this first slice. Those issues remain historical until separately re-triaged; do not recreate their document questionnaire, Cabinet-first Memory scope or provider-choice assumptions here.
- Blocked until the documentation reset is merged from `codex/product-direction-reset` and the reusable PR #61 infrastructure has a reviewable extraction location.
- PR [#61](https://github.com/yasserchehade/Luna/pull/61) must remain unmerged and close as superseded once its replacement extraction work is reviewable.
