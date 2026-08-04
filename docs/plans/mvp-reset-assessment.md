# Luna MVP reset assessment

> **Historical assessment note (2026-08-04):** This assessment predates the web-first decision in ADR 0020. Its Household Work, OpenAI and Luna-owned authority findings remain useful. Desktop retention, local persistence, Cabinet, portable-memory and email-first sequencing are superseded by the [web-first migration assessment](./web-first-migration-assessment.md).

**Review date:** 2026-08-03

**Reviewed:** repository `HEAD` `6ec3c21` on `codex/issue-38-managed-conversation`, the current product and architecture documentation, and open PR #61 (`Add default intelligence permissions and managed conversation replies`, head `codex/issue-38-managed-conversation`, base `main`).

## Executive assessment

The repository has valuable security, persistence, attachment and OpenAI transport foundations, but the current product path does not yet prove the agreed MVP. It still treats Document Handling as the durable centre, routes ordinary Conversation and document interpretation through separate paths, and intentionally limits ordinary Conversation to the newest message. PR #61 improves the managed OpenAI route and attachment transport, but it formalises those boundaries rather than resolving them.

The reset should preserve infrastructure and replace the product boundary. Do not merge PR #61 as-is. Split reusable infrastructure from the superseded product work, close the current product-shaped PR once the salvageable pieces are extracted, and replace it with the household-work agent sequence in [the follow-up plan](./mvp-reset-follow-up-issues.md).

## Classification

### Retain

| Capability | Evidence | Decision |
| --- | --- | --- |
| Account authentication and household identity | `desktop/src/account/`, Supabase account contract, trusted-device flows | Retain as the identity and authority foundation. |
| Trusted-device and credential-vault handling | `desktop/src-tauri/src/trusted_device.rs`, managed gateway provisioning | Retain. OpenAI access must remain behind Luna-owned credentials and permissions. |
| Conversation UI, composer and durable messages | `desktop/src/conversation/ConversationWorkspace.tsx`, `conversationService.ts`, conversation store | Retain the interaction and persistence seam; change the routing and context contract. |
| Attachment transport and original preservation | `attach_document` in `desktop/src-tauri/src/lib.rs`, `document_attachment_for_arrival` in `intelligence.rs`, attachment chronology tests | Retain and reuse for email attachments and the member attachment path. |
| Durable local persistence and audit/history | conversation store, portable memory, audit and execution history | Retain the persistence, audit and recovery controls; change the owning domain to Household Work. |
| OpenAI Responses/LiteLLM transport | `desktop/src-tauri/src/litellm.rs`, managed OpenAI route and synthetic deterministic gateway | Salvage as a transport seam. It must accept the shared context-aware agent contract. |
| Untrusted-result validation and bounded retry | `intelligence.rs`, `litellm.rs`, domain validation and failure mapping | Retain. Extend validation to work proposals and tool calls. |
| Local parsing/OCR and Cabinet original preservation | `document_intelligence.rs`, Cabinet handling, PDF/OCR support | Retain as supporting source/evidence and recovery capabilities, not as the product centre. |
| Testing infrastructure and installed-app evidence | Rust, TypeScript, E2E and build-flavor seams | Retain and redirect tests toward email-to-work and monitored outcomes. |

### Simplify

| Capability | Current shape | Simplification |
| --- | --- | --- |
| Document review and context extraction | Large typed `ReviewCard`, field-level `DocumentContextReview`, clarification states and filing forms | Treat extracted facts as evidence on Household Work. Keep a compact correction/transparency surface; remove field completion as the default path. |
| Document lifecycle | `DocumentProcessingState` contains consent, local inspection, filing, duplicate and Cabinet states | Keep source preservation and filing support, but make it subordinate to a work lifecycle: observed, understood, awaiting input/approval, in progress, monitoring, completed, dismissed or irrelevant. |
| Conversation orchestration | `ConversationPrompt` is derived from document states and routes replies to field directions | Derive explanations, questions and approvals from active Household Work. A member reply should not be intercepted by a document questionnaire. |
| Intelligence gateway | Provider-neutral catalogue, capability-specific schemas and document field disclosure are more general than MVP needs | Keep one Luna-owned OpenAI route and the untrusted-result boundary. Hide provider choice and postpone the generalized catalogue. |
| Options and managed access | Options includes entitlement, provisioning, provider status and billing affordances | Keep a small managed OpenAI status surface for transparency and operations. Remove provider configuration from the primary product experience. |
| Cabinet and filing rules | Cabinet, filing decisions, duplicate handling and learned filing rules are treated as the first product value | Keep them as record/evidence competencies. Do not make them prerequisites for proving email-to-work-to-action. |
| Portable memory | Extensive encrypted synchronization for document rules, consent and history | Retain security and recovery foundations, but add only the minimum portable Household Work facts required by the MVP. Defer broad portability requirements. |

### Disable or defer

- local-only intelligence and local models;
- multiple Intelligence Providers and provider-selection UX;
- BYOK and provider-key configuration;
- complex one-time or per-document consent grants in the main experience;
- generic workflow-builder functionality;
- autonomous bill payment and bank integrations;
- broad role and authority systems beyond the existing safe household boundary;
- advanced filing-rule learning and automatic filing as the main success metric;
- extensive live billing infrastructure and Paddle product work;
- separate provider canaries and release gates for deferred routes; and
- infrastructure abstractions added only for hypothetical provider or integration requirements.

These are not deleted from history. They are out of MVP scope until the end-to-end household-administration loop is proven.

### Replace

| Conflicting path | Evidence | Replacement |
| --- | --- | --- |
| Ordinary conversation receives only the latest message | `desktop/src-tauri/src/intelligence.rs:1187-1241` builds `content_excerpts` from `CURRENT_MESSAGE_FIELD` only; the test at `:2083-2138` asserts that contract | One context-aware household-administration request containing relevant recent conversation, email/document source, household context and active work. |
| Attached document is optional context only for the current turn | `lib.rs:739-765` resolves only an optional linked Document Arrival for `reply_to_conversation_with_document` | Source ingestion and active work become first-class context. The model reads email and attachment together and relates them to the household. |
| Separate ConversationReply and DirectionInterpretation paths | `intelligence.rs:88-91`, `document_intelligence.rs:71-190`, and `ConversationWorkspace.tsx:772-789` split ordinary messages from prompted document direction | One reasoning layer with shared proposals. Deterministic inspection and review are supporting evidence, not a second intelligence path. |
| Typed document prompts intercept the composer | `ConversationWorkspace.tsx:772-782` selects a prompted arrival and calls `submitUtterance` instead of ordinary conversation | Route the member's natural reply using active Household Work context. Ask a question conversationally only when evidence is exhausted. |
| Members are asked to fill information already in source evidence | `ConversationWorkspace.tsx:430-455` renders inputs for provider, property, account, amount and dates; `:457-460` exposes clarification questions | Pre-fill and reason from email/attachment/household context. Show fields only for correction or transparency. |
| Document Handling is the durable domain centre | `docs/domain-model.md` previously named Document Handling as the core domain; current code centres `DocumentArrival` and `DocumentProcessingState` in `conversation.rs:41-52` | Make Household Work / internal `Obligation` the durable owner. Documents and attachments become sources and evidence. |
| To-do is a projection of document handling | The current glossary and `TodoItem` path link unresolved action directly to a Document Arrival | Project attention from Household Work so email updates, replies, reminders and documents stay on one item. |
| Provider and consent concepts shape product complexity | `intelligence.rs:31-45`, capability-specific consent scopes and `CloudAssistanceOptions.tsx:131-174` | Use a managed OpenAI route for MVP; retain Luna's privacy and execution boundary without making provider settings the user's work. |
| Review cards and forms remain part of the completion route | `ConversationWorkspace.tsx:411-477` mounts the review editor with context and filing forms | Keep Review details as secondary correction/transparency. The primary completion route is natural conversation plus validated actions. |
| MVP has no household email source yet | Repository search finds account-verification email fixtures, but no Gmail, Outlook, IMAP, mailbox or inbox intake implementation | Add one authorised household email source before claiming the MVP loop is proven. |

## PR #61 recommendation

**Recommendation: split PR #61 into reusable infrastructure and superseded product work.** It should not be merged as-is.

PR #61 contains salvageable work:

- managed OpenAI Responses transport;
- original PDF/JPG/PNG attachment transmission;
- secure gateway credential provisioning;
- bounded response validation and failure handling;
- durable conversation persistence and attachment chronology; and
- installed-app and deterministic test seams.

It also makes product decisions that conflict with the reset:

- `ConversationReply` is explicitly limited to the newly submitted message;
- `DirectionInterpretation` remains a separate document path;
- the reasoning contract has no household or active-work context;
- the durable model remains document-led;
- the code retains capability-specific provider/consent concepts; and
- the main document route still exposes typed facts and correction forms.

The clean action is to extract the infrastructure into a focused follow-on change, close the current PR once that extraction is complete, and open a replacement PR implementing the shared context-aware Household Work agent. If extraction cannot be made small and unambiguous, close #61 and cherry-pick only reviewed infrastructure commits into the replacement branch.

## Recommended follow-up issue order

1. Freeze and split #61; inventory salvageable infrastructure and mark conflicting product work superseded.
2. Define the durable Household Work / `Obligation` model and lifecycle.
3. Connect one household email account and ingest email plus attachments idempotently.
4. Assemble relevant household context, including recent conversation and active work.
5. Replace the latest-message-only and separate document contracts with one OpenAI agent contract.
6. Create/update work from an electricity-bill email without asking for known facts.
7. Make Conversation the work interaction layer and remove document-prompt interception.
8. Add validated draft reply/reminder proposals, approval and Luna-owned execution.
9. Monitor work through completion, dismissal or irrelevance and recover safely.
10. Add full-loop installed-app tests, then retire or hide superseded document-first surfaces.

The detailed acceptance intent for these issues is in [mvp-reset-follow-up-issues.md](./mvp-reset-follow-up-issues.md).
