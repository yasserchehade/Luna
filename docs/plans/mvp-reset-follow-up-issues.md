# MVP reset follow-up implementation issues

**Status:** Recommended sequence; GitHub issues not yet created

This sequence turns the product reset into a small set of dependency-aware implementation issues. It supersedes the old document-first order in `luna-first-vertical-tickets.md` for future planning. The old ticket map remains historical and should not be implemented by default.

The first detailed issue is [Unified uploaded-document Household Work journey](./unified-uploaded-document-household-work.md), tracked in [GitHub issue #73](https://github.com/yasserchehade/Luna/issues/73). It is intentionally an uploaded-document slice; Gmail and Outlook are not dependencies.

## Sequence

### 1. Freeze the conflicting product path and split reusable infrastructure

Review PR #61 against the reset. Keep or extract secure OpenAI transport, Responses attachment support, credential-vault handling, persistence, validation, audit and deterministic test seams. Mark latest-message-only conversation, separate document evaluation, document prompts and provider-permission UX as superseded product work.

Use `codex/product-direction-reset` for the documentation PR, then `codex/pr61-infrastructure-extraction` from the post-reset `main` for the salvage PR. PR #61 remains open only until the salvage PR is reviewable, then closes without merging.

### 2. Define Household Work / `Obligation`

Introduce the minimum durable work model: source references, summary, facts, responsibility, due date, urgency, lifecycle, proposed action, approval, execution, monitoring and audit. Define how legacy `Document Handling` records project into or are retired behind the new work owner.

### 3. Add the first household email source

Connect one authorised household email account. Ingest sender, recipients, subject, body, received time and PDF/JPG/PNG attachments. Preserve source identity and attachment bytes, make ingestion idempotent, and keep source failures visible in household work.

### 4. Assemble relevant household context

Build one Luna-owned context assembler for current message, recent relevant conversation, email, attachment, confirmed household subjects, prior decisions, active work, responsibility and authority. Add disclosure evidence and tests proving that useful context is not reduced to the latest sentence.

### 5. Replace split reasoning with one OpenAI agent contract

Define a context-aware OpenAI request and untrusted result that can explain, extract facts, propose work changes, ask a question and propose typed tool calls. Keep Luna-owned correlation, schema validation, authority and execution boundaries.

### 6. Create and maintain work from incoming information

Use the agent to decide whether a source needs attention, then create or update one durable work item. Prove the electricity-bill path: provider, amount, property, account, due date, urgency and next action are available without member re-entry.

### 7. Make conversation the work interaction layer

Remove typed document prompts from the primary composer path. Route member replies through the active Household Work context, preserve recent relevant turns, explain the work naturally and show structured facts only as secondary correction/transparency surfaces.

### 8. Add safe proposals, approvals and simple execution

Implement draft reply and reminder proposals. Add Luna-owned approval, validation, idempotency, execution and audit for the smallest useful action set. Ensure OpenAI cannot execute or grant approval.

### 9. Monitor work to a terminal outcome

Track due dates, waiting states, follow-ups, completion, dismissal and irrelevance. Update the same work item when a later email confirms completion or requires a response; do not create duplicate tasks for each message.

### 10. Prove the complete MVP loop and retire superseded surfaces

Add installed-app and backend tests for email-to-work, conversation context, attachment reading, no-repeat questioning, approval, failure/retry, monitoring and restart recovery. Then remove or hide document-first forms, provider configuration and legacy paths that no longer serve the MVP.

## Dependency rule

An issue is ready only when its product purpose, durable owner, context requirements, authority boundary, failure state and user-facing conversation outcome are specified. No issue in this sequence should add a general integration framework or future provider abstraction unless it is required by the next end-to-end household-administration test.
