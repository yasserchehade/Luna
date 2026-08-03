# Luna MVP Definition

**Status:** Binding MVP scope

The MVP proves that Luna can take ownership of a small but real slice of modern family administration. It is intentionally narrower than the long-term product and intentionally broader than a document questionnaire.

## Exact MVP problem

Household administration arrives as fragmented email, attachments and follow-up decisions. A family member must notice the message, open the attachment, understand what it means, remember the relevant property or account, decide who should act, create a reminder or response, and keep checking until the matter is resolved.

The MVP problem is:

> A household needs Luna to turn relevant email and attachments into durable household work, explain that work naturally, and help move it to a safe outcome without making a member repeat facts already present in the source or household context.

The MVP is successful when it reduces the work of handling a household obligation, not merely when it extracts document fields.

## Target user

The target user is a modern family using one connected household email account. The first account may be operated by a Household Organiser, but the model must preserve the distinction between household responsibility and access to another adult member's private information.

## First end-to-end workflow

1. The household connects an email account authorised for household administration.
2. Luna observes and retrieves household-related email.
3. Luna reads the email body, metadata and supported attachments.
4. OpenAI reads and interprets the email and attachment using relevant recent conversation and household context.
5. Luna determines whether the information creates, updates, completes, dismisses or leaves no household work.
6. Luna extracts relevant facts such as sender, provider, amount, due date, property, account, responsible member, required action and urgency.
7. Luna creates or updates one durable household work item. Its internal domain name may be `Obligation`.
8. Luna explains the work in natural conversation, linking the source and the evidence that matters.
9. Luna proposes a safe next action such as a draft reply, reminder or request for approval.
10. Luna validates any proposed tool call, asks for approval where appropriate, executes only within its authority, records the outcome and keeps monitoring the work.
11. The work remains active until it is completed, dismissed or no longer relevant.

## Supported inputs

The MVP supports:

- one connected household email account;
- email sender, recipients, subject, body and received time;
- PDF, JPG and PNG attachments from relevant email;
- recent relevant conversation with the household member;
- household context already confirmed in Luna, including members, properties, accounts, providers and responsibilities;
- prior decisions and active household work relevant to the source; and
- a member's natural-language message, with an optional attached document for development and recovery paths.

The email account is the first source of incoming work. Gmail, Outlook and future sources are source adapters; they are not separate product experiences or separate reasoning systems.

## Expected outputs

For relevant incoming information Luna should produce:

- a durable household work item with a clear human-readable summary;
- the source reference and preserved attachment where applicable;
- extracted facts with evidence and uncertainty where needed;
- the responsible household member or a focused question if responsibility is genuinely unclear;
- urgency and due-date state when supported by evidence;
- a natural conversational explanation;
- a proposed next action or a statement that no action is needed;
- a draft reply, reminder or other simple prepared action where useful;
- an approval request before consequential external action;
- an audit trail of interpretation, decisions, proposed actions and executions; and
- a monitoring state until the work reaches a terminal outcome.

For irrelevant or already-resolved information Luna should avoid creating noisy work and should be able to explain why it did not create or retain an active item.

## Initial agent capabilities

The MVP agent can:

- observe new household email and attachments;
- understand email and documents together using OpenAI;
- assemble relevant household and conversation context;
- identify whether attention is needed;
- extract sender/provider, amount, due date, property, account, action and urgency;
- create, update, merge or close durable household work when the evidence supports it;
- explain work in natural language without requiring a form;
- ask one focused question after exhausting available evidence;
- prepare a draft reply or reminder;
- propose Luna-owned tool calls;
- request approval for actions beyond standing authority;
- execute a small approved action through a Luna-owned tool seam;
- record decisions, execution outcomes and recovery state; and
- follow up on an active item until it is complete, dismissed or no longer relevant.

The MVP agent does not make autonomous payments, change bank state, invent authority or claim that a provider result is an executed action.

## Minimum durable data

The minimum durable domain is household work, not a document record. An internal `Obligation` should carry at least:

- stable identifier and household ownership;
- lifecycle state: active, awaiting member input, awaiting approval, in progress, monitoring, completed, dismissed, no longer relevant or blocked;
- source references, including email and attachment identifiers;
- concise summary and work type;
- extracted facts with evidence, confidence or uncertainty;
- provider, property, account, member and responsibility references where known;
- amount, due date, urgency and required action where relevant;
- linked conversation and message references;
- proposed action and its approval status;
- execution status, result and retry/recovery information;
- created, updated, due and closed timestamps; and
- immutable audit references for important interpretation, direction, approval and execution events.

Documents and attachments remain durable sources and originals. They may be preserved for evidence, filing and recovery, but they do not own the household work lifecycle.

## Approval boundaries

Luna may automatically:

- ingest an authorised email;
- read an authorised email and attachment through the MVP OpenAI route;
- persist a source reference, extracted evidence and a household work item;
- update work state when the event is unambiguous and within existing authority;
- prepare a draft reply or reminder; and
- schedule or send a low-risk action only when an explicit standing direction authorises it and Luna's validation passes.

Luna must request member approval before:

- sending an external reply without applicable standing authority;
- making a consequential appointment, cancellation, purchase or commitment;
- changing a provider, account or property record in an external service;
- sharing household information with a new recipient or service; or
- taking an action whose scope, target or authority is ambiguous.

OpenAI may propose an action or tool call. It may not authenticate, select permissions, mutate durable state, execute a tool or grant approval.

## Explicit non-goals

Defer the following until the MVP loop is proven:

- local models and local-only reasoning;
- multiple intelligence providers;
- bring-your-own-key support;
- provider-selection UX;
- complex per-document consent grants;
- generic workflow-builder functionality;
- autonomous bill payment;
- bank integrations;
- broad role and authority systems;
- advanced filing-rule learning;
- extensive billing infrastructure; and
- architecture built solely for hypothetical future requirements.

The MVP may retain secure seams that make these possible later, but they must not dominate implementation or the member experience.

## Success criteria

The MVP is ready for the next phase when all of the following are demonstrated:

1. A relevant email and attachment create or update one durable household work item without a member retyping facts visible in the source.
2. Luna can explain the work conversationally, including what it found, why it matters, what it proposes and what it needs.
3. The reasoning request includes the relevant attachment, recent relevant conversation and household context required to interpret the item; it is not limited to an isolated latest sentence.
4. The same work item remains coherent across a new conversation turn, reload, restart and source follow-up.
5. A simple reply or reminder can be prepared, approval can be requested where required, and the result is recorded with an audit trail.
6. Luna asks a member only after available email, attachment, context, prior decisions and conversation evidence have been exhausted.
7. An ambiguous, failed or unavailable action waits safely and can resume without duplication or silent fallback.
8. The member sees outcomes and concise explanations first; structured facts and review controls are secondary.
9. No OpenAI response can bypass Luna's validation, authority, execution, persistence or recovery boundary.
10. Tests cover the full household-administration loop, not only document field extraction or provider transport.

## Representative user journeys

### Electricity bill from email

An electricity provider sends a bill by email. Luna reads the email and attachment, identifies the provider, amount, property and due date, creates household work, explains it conversationally, and offers an appropriate next action without asking the user to repeat information from the bill.

The member can reply, "Remind me three days before it is due," and Luna understands the reference from the active work item. Luna prepares the reminder, confirms its schedule and keeps the item active until the reminder is delivered or the bill is otherwise resolved.

### Ambiguous responsibility

An insurance renewal names the household but does not establish which adult should approve the change. Luna records the renewal, explains the evidence and asks which member should decide. It does not ask for the policy number or provider when those are already present in the email and attachment.

### Drafted provider response

A provider asks the household to confirm an address. Luna identifies the request, checks the household's confirmed property context, drafts a concise reply and presents the target and proposed content. Luna sends only after the member approves, then records the sent result and monitors for a response.

### Already resolved or irrelevant mail

An email confirms that a previously tracked repair is complete. Luna links it to the active work, marks the work completed and explains the closure. A promotional message with no household action becomes no active work item and does not create a distracting task.

### Provider follow-up

Luna sends or prepares an approved request and receives no response by the agreed date. It keeps the same household work item, explains that the matter is waiting, and proposes a follow-up rather than creating an unrelated duplicate.
