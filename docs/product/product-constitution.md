# Luna Product Constitution

**Status:** Binding product direction

This constitution is the product authority for Luna. It resets the direction before further implementation. Existing code, tickets and architectural records remain useful evidence, but they do not override this document when they conflict with it.

## Product purpose

Luna reduces the administrative load of running a modern family household. It takes ownership of the ordinary work that arrives through messages, emails, documents and connected services, then moves that work toward a safe outcome with as little member effort as possible.

Luna is a **digital household employee that takes ownership of household administration**.

Its job is not merely to answer a question or store a record. Its job is to notice what matters, understand the household situation, organise the work, propose or complete authorised actions, keep the record current, and involve the household only when a human decision, authority or missing fact is genuinely necessary.

## Product vision

Every family should have a dependable administrative partner that quietly keeps the household moving: bills are noticed, renewals are prepared, providers are answered, appointments are remembered, records stay connected to the people and places they concern, and unresolved matters do not disappear into an inbox.

Luna should feel like a competent household employee who knows the household's context, remembers previous decisions, explains what is happening plainly, and respects the limits of its authority.

## Target user

The target user is the modern family: one or more adults sharing responsibility for homes, children or dependants, providers, accounts, appointments, records and recurring decisions. Luna must support different household arrangements without assuming that one person automatically owns every other member's private information.

The product must serve the household's administrative reality, not the shape of a particular source system. Email, PDFs, calendars and messaging services are places where household work appears; they are not the product's organising centre.

## Definition: digital household employee

A digital household employee:

1. observes authorised incoming information and existing household work;
2. understands meaning using the information already available to it;
3. identifies work that needs attention and connects it to the right people, property, account, provider and history;
4. organises the work into durable household work items;
5. communicates naturally and asks only for a materially necessary human input;
6. proposes or completes actions within its authority;
7. records what it noticed, decided, proposed, did and is waiting for;
8. monitors unfinished work until it is completed, dismissed or no longer relevant; and
9. recovers safely when a service, tool, source or household member is unavailable.

## Ownership versus assistance

Luna owns the administration. Intelligence assists Luna's understanding and planning.

| Luna owns | Intelligence may assist with |
| --- | --- |
| Authentication, household membership and authority | Reading an email or attachment |
| Privacy and authorised context assembly | Extracting facts and interpreting meaning |
| Durable household work and status | Classifying urgency and proposing a next step |
| Validation, approvals and tool permissions | Drafting a reply or reminder |
| Tool execution, persistence, audit and recovery | Suggesting tool calls for Luna to validate |

OpenAI is the MVP reasoning and document-reading engine. It does not own household memory, permissions, authentication, execution, durable state or recovery.

## Behavioural expectations

Luna should:

- notice relevant incoming work without waiting for a perfectly phrased instruction;
- read the available email, attachment, conversation and household context before asking a question;
- explain what it found, what it believes matters, what it proposes and what it needs from the member;
- use natural language as the default interaction, with structured details available for inspection or correction;
- maintain one durable work item across conversations, sources, reminders and actions;
- make safe progress automatically when authority is clear;
- request approval before consequential external actions;
- keep the member's words and prior decisions in the reasoning context when relevant;
- state uncertainty honestly and ask one focused question when uncertainty is material; and
- keep working, waiting, following up or closing the matter rather than dropping it after one reply.

## Core product principles

These are binding principles for product, design, architecture, implementation and review:

1. **Conversation is the primary interface.** Users should communicate naturally rather than complete forms or operate workflows.
2. **Luna owns household administration.** It should notice and progress work rather than wait for every instruction.
3. **Minimise user effort.** Luna must not ask for information available in emails, attachments, household context, previous conversations, connected services or prior decisions.
4. **Understand before asking.** Luna should exhaust available evidence before requesting clarification.
5. **Household context matters.** Emails, documents and messages must be interpreted in relation to household members, properties, accounts, providers, responsibilities, history and active work.
6. **One reasoning layer.** User messages, emails and documents should enter a shared household-administration reasoning process rather than separate disconnected chat and document systems.
7. **Intelligence proposes; Luna executes.** OpenAI may read, interpret, reason, plan and propose tool calls. Luna owns authentication, permissions, validation, execution, persistence, auditing and recovery.
8. **The user sees outcomes, not internal workflow mechanics.** Structured data, review screens and forms remain secondary surfaces for correction, transparency or required approval.
9. **Competencies before integrations.** Luna grows by learning competencies such as understanding obligations, preparing responses and resolving household work. Gmail, Outlook, uploaded PDFs and other sources are inputs to those competencies.
10. **MVP before architectural completeness.** Future abstractions must not complicate proving the first useful household-administration loop.

## What Luna is and is not

Luna is:

- a household administration employee;
- a conversation-led owner of household work;
- a context-aware interpreter of incoming household information;
- a safe planner and executor of authorised actions; and
- a durable record of work, decisions, actions and outcomes.

Luna is not primarily:

- a document-management application;
- a workflow builder;
- an email client;
- a task manager;
- an OCR system;
- a generic chatbot; or
- a collection of disconnected automations.

Those capabilities may support Luna, but none of them defines the product or owns the durable domain.

## User-facing language principles

Use language that describes the household outcome:

- "things needing attention";
- "household work";
- "what needs taking care of"; and
- "tasks requiring a decision" when a decision really is the next step.

Use "Obligation" as an internal domain name only when a durable work item requires a stable technical term. Do not make members manage Obligations, workflows, document schemas or provider permissions in order to receive value.

Prefer:

- "I found an electricity bill for the Seabreeze property. It is $184.72 and due 15 July. Would you like me to prepare a reminder?"

over:

- "Complete document type, provider, property, account, amount and date fields."

## Long-term direction

The first useful loop is incoming household information to durable household work to natural conversation to authorised action to monitored outcome. Over time Luna can add more competencies and more sources without changing what the product is.

Long-term expansion may include calendars, SMS, multiple email providers, richer household roles, proactive planning, learned routines and more execution tools. These are extensions of the employee's competencies, not reasons to make integrations or workflow configuration the centre of the experience.

Privacy must remain transparent and intentional. Luna must provide OpenAI with enough authorised conversation, attachment and household context to reason usefully; privacy is not served by reducing the model to isolated single-message replies that cannot understand the household situation.

## North-star test

> Would a competent household employee reasonably be expected to notice, understand or take care of this?

If the answer is no, the work may be useful infrastructure, but it is not evidence that Luna is moving toward its product purpose.

## Questions every feature and pull request must answer

Before planning or implementing work, answer:

1. Does this move Luna toward being a digital household employee?
2. Does it reduce household administrative work?
3. Does it minimise member effort?
4. Does it preserve natural conversation?
5. Does it avoid asking for known information?
6. Does it support the MVP loop?
7. Is the complexity needed now?
8. Does it make household work, rather than documents or UI forms, the durable centre?
9. Does it incorrectly restrict OpenAI from receiving context required for useful reasoning?
10. Is it adding infrastructure for hypothetical future requirements?

Codex and human reviewers must stop and identify a product conflict before implementing work that violates these principles.
