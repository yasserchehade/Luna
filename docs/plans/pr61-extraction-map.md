# PR #61 extraction map and closure strategy

**Status:** Implementation planning; do not merge PR #61 as-is

**Reviewed:** PR [#61](https://github.com/yasserchehade/Luna/pull/61), base `main`, head `codex/issue-38-managed-conversation` at `6ec3c21`, 29 commits from merge-base `8adf762`.

## Documentation reset isolation

The current working-tree changes are documentation and GitHub workflow-template files only. No Rust, TypeScript, frontend, backend, generated runtime schema or application configuration file is changed by the reset. PR #61 itself contains application changes; the reset must not be merged through that branch.

The documentation reset should be merged independently:

1. Create `codex/product-direction-reset` from `main`, not from `codex/issue-38-managed-conversation`.
2. Commit only the product, architecture, domain, planning, historical-ADR, security/testing-notice, `AGENTS.md`, README, `CONTEXT.md` and issue/PR-template changes.
3. Open one PR titled `Document the household-administration product reset` against `main`.
4. Require documentation review and `git diff --check`; do not require a desktop build because the PR has no runtime changes.
5. Merge this PR before opening replacement implementation PRs. The reset PR must not depend on PR #61 and must not cherry-pick its product commits.

The current uncommitted documentation changes are sitting on the PR #61 branch because the reset was prepared after that branch was checked out. They should be moved onto the clean `codex/product-direction-reset` branch before commit; do not commit them into PR #61.

## Classification vocabulary

- **Retain unchanged:** safe to cherry-pick as a product-neutral fix or stable operational boundary.
- **Retain after simplification:** salvage a seam or file-level change, but do not carry the old product contract or UX.
- **Obsolete product behaviour:** directly implements the superseded split, provider UX, document questionnaire or latest-message-only contract.
- **Unrelated fix:** useful in the old branch but not a dependency of the first unified Household Work slice.
- **Unsafe to extract independently:** mixes reusable infrastructure with obsolete product behavior, generated artifacts or coupled tests; port selected hunks manually after the new boundary exists.

## Commit-level map

| Commit | Meaningful change | Classification | Extraction action |
| --- | --- | --- | --- |
| `b6473d3` | Adds `ConversationReply`, default provider/permission state, ordinary reply persistence, current-message-only request, managed conversation UI and related tests/docs | Unsafe to extract independently; mixed obsolete product behavior | Do not cherry-pick. Reuse only the `add_luna_message`, managed route and response-validation ideas while implementing the new agent contract. |
| `bea9b63` | Makes stale default-provider controls recoverable in Options | Obsolete product behaviour | Do not extract; provider/default UX is deferred. |
| `dafdeb9` | Prevents unreadable consent history from disabling access | Retain after simplification | Keep only if legacy records must remain readable; it is not a first-slice dependency. |
| `c16d093` | Shows verified permissions when historical consent rows are unreadable | Obsolete product behaviour | Do not extract into the MVP; preserve only as compatibility evidence if needed. |
| `7618954` | Proves/repairs managed conversation access, stale credential provisioning and live-canary plumbing | Retain after simplification; unsafe as a whole | Salvage managed credential recovery and its deterministic seam. Exclude live-canary application bridges and Conversation-specific access UX. |
| `500c799` | Makes test-build feature flags compose in the Tauri entry point | Retain unchanged | Cherry-pick if the clean branch still needs the same test-build composition. |
| `7c7ce54` | Records founder managed-route proof in operations/planning docs | Unrelated fix | Do not cherry-pick; retain as historical evidence only. |
| `d448f05` | Closes PR review gaps across managed replies, permissions, attachment tests, account fixtures, frontend and docs | Unsafe to extract independently | Do not cherry-pick. Port only isolated security or transport fixes after the new agent boundary is defined. |
| `62b3ded` | Moves the native document picker off the Tauri main thread | Retain unchanged | Cherry-pick. It is a product-neutral attachment transport fix. |
| `442528c` | Locks the picker threading regression with a compile-time test placement | Retain unchanged | Cherry-pick with `62b3ded`. |
| `0363e89` | Prunes unavailable or ineligible duplicate Originals | Unrelated fix | Do not extract into the first slice; reconsider if document-source deduplication becomes a concrete blocker. |
| `f8f9b20` | Stops stale recovery warnings and skips unreadable historical cloud audit rows | Unrelated fix; partly retain after simplification | Do not cherry-pick as a unit. Preserve credential/recovery fixes only when independently needed. |
| `46ef8ca` | Revalidates duplicate reviews when Cabinet state changes | Unrelated fix | Do not extract into the first slice. |
| `9e06bd8` | Backfills missing legacy current-epoch device keys | Retain unchanged | Cherry-pick as a security/recovery fix if not already present on the clean base. |
| `ab35a4f` | Adds durable attachment timeline anchors and preserves upload chronology | Retain unchanged | Cherry-pick; chronology remains useful for uploaded-document Conversation. |
| `e3f612c` | Orders legacy unanchored attachments before new anchors | Retain unchanged | Cherry-pick with `ab35a4f`. |
| `1fe5d88` | Adds the synthetic GPT-5.6 Luna Responses canary and managed operator scripts | Retain after simplification | Keep the synthetic managed-route contract only; move research/docs out of the implementation PR. |
| `3936135` | Switches managed and BYOK routes to GPT-5.6 Responses, changes provisioning and canaries | Unsafe to extract independently | Do not cherry-pick whole. Port the managed OpenAI Responses route, credential provisioning and bounded route checks; exclude BYOK and document-only contracts. |
| `a6bec10` | Fixes BYOK Responses schema | Obsolete/deferred | Do not extract. |
| `e71c356` | Makes BYOK canary probes cache-safe | Obsolete/deferred | Do not extract. |
| `4ac5437` | Probes BYOK authentication before provider use | Obsolete/deferred | Do not extract. |
| `f9a1a5c` | Records BYOK rate-limit canary evidence | Obsolete/deferred | Do not extract. |
| `7bc2386` | Removes BYOK/local-provider MVP exposure and focuses on managed Luna | Retain after simplification | Keep the decision to use one managed OpenAI route; do not carry the provider/Document UI shape. |
| `fac8dc7` | Recovers stale managed gateway access in the desktop coordinator | Retain unchanged | Cherry-pick if the managed credential recovery seam is not already on the clean base. |
| `a721563` | Constrains document interpretation fields and dates | Obsolete product behaviour; validation idea reusable | Do not cherry-pick. Reapply value bounds to the new Household Work contract, which must support proposals rather than document fields. |
| `71b3c8f` | Simplifies managed intelligence and attaches documents in chat | Unsafe to extract independently | Salvage attachment selection/persistence and managed-route plumbing; reject document-prompt and separate evaluation behavior. |
| `8dc8edc` | Sends preserved original PDFs/images to managed OpenAI | Retain after simplification | Port the original-attachment transport into the unified request; do not preserve `DocumentIntelligenceService` as a separate agent. |
| `7692384` | Raises managed document throughput limits and provisioning caps | Retain after simplification | Keep capacity changes only where needed for the unified uploaded-document request; remove document-specific acceptance language. |
| `6ec3c21` | Adds OpenAI household-admin conversation research | Retain as research | Keep the research record, but treat the new product/architecture documents and ADR 0019 as governing. |

## File and subsystem extraction map

| Subsystem | PR #61 files | Decision |
| --- | --- | --- |
| OpenAI/LiteLLM transport | `desktop/src-tauri/src/litellm.rs`, `desktop/src-tauri/src/intelligence.rs`, `ops/litellm/*`, `supabase/functions/*` | Retain after simplification. Keep managed OpenAI Responses transport, credentials, route pinning, bounded retries, usage metadata and untrusted-result validation. Replace capability-specific document/current-message schemas with the shared agent contract. |
| Managed credential handling | `desktop/src/account/managedIntelligenceCoordinator.ts`, `desktop/src/trusted-device/*`, `desktop/src-tauri/src/trusted_device.rs`, `desktop/src-tauri/src/lib.rs`, account-service tests | Retain unchanged where it is route authentication, provisioning, renewal, revocation or recovery. Do not expose provider selection or BYOK setup in the MVP. |
| Conversation persistence | `desktop/src-tauri/src/conversation.rs`, `desktop/src/conversation/conversationService.ts` | Retain after simplification. Keep durable messages and `add_luna_message`; add Household Work references and context loading. Do not keep ordinary messages as isolated provider requests. |
| Attachment support | `desktop/src-tauri/src/lib.rs`, `conversation.rs`, `ConversationWorkspace.tsx`, `conversationTimeline.ts`, `conversationTimeline.test.ts` | Retain. Picker threading, original preservation, attachment anchors and chronology are directly useful. The attachment becomes a source for Household Work rather than a trigger for a questionnaire. |
| Request/response validation | `intelligence.rs`, `litellm.rs`, Rust conversation tests | Retain after simplification. Preserve correlation, schema, size, retry and failure checks; define validation for reply, work proposal, clarification and proposed action. |
| Provider/default-permission UX | `CloudAssistanceOptions.tsx`, `conversationService.ts`, `App.tsx`, Options tests | Obsolete product behaviour. Replace with transparent managed OpenAI status only; no provider choice, BYOK, per-request permission switch or per-document consent flow. |
| Ordinary conversation submission | `lib.rs`, `intelligence.rs`, `ConversationWorkspace.tsx`, `conversationService.ts` | Obsolete product behaviour. Replace with one context-aware submission that includes relevant conversation, household context, active work, source attachment and tool definitions. |
| Document intelligence | `document_intelligence.rs`, `conversation.rs`, review-card types and tests | Obsolete as a separate path. Preserve local parsing, original bytes and evidence utilities only as supporting source handling. |
| Typed prompt interception | `ConversationWorkspace.tsx`, `conversation.rs`, `submit_member_utterance` path | Obsolete product behaviour. Natural user replies must be interpreted against active Household Work; no field questionnaire under a new name. |
| Tests | `desktop/e2e/conversation.spec.ts`, `desktop/src-tauri/tests/conversations.rs`, Options/live-canary tests | Retain test seams; replace assertions that prove current-message-only requests, document-field completion or provider permissions with unified agent acceptance scenarios. |
| Documentation and operations | `CONTEXT.md`, ADR 0015, gateway/operations/testing/planning docs | Mark superseded or rewrite against product reset. Do not cherry-pick PR #61 documentation as a source of truth. |

## Definitive PR #61 disposition

PR #61 should remain open temporarily, frozen, while:

1. the documentation reset is moved to and merged from `codex/product-direction-reset`;
2. the clean extraction PR is opened with only product-neutral fixes and manually ported managed OpenAI infrastructure; and
3. the first unified Household Work issue is created and linked.

PR #61 must not be merged. Once the replacement infrastructure PR is open and the selected commits are either cherry-picked or explicitly ported, close PR #61 with a comment stating that it is superseded by the product reset and linking the replacement PR(s) and first-slice issue. Do not close it before the salvage location is reviewable.

The replacement work should be split into small PRs:

1. documentation reset;
2. product-neutral attachment, credential and runtime-stability extraction;
3. Household Work data model and context-aware agent contract;
4. uploaded-document OpenAI reasoning and Luna validation;
5. natural Conversation presentation, correction, dismissal and acceptance evidence.

The final state must contain no merge of PR #61's default-permission UX, current-message-only contract, separate document-intelligence workflow or typed document prompt interception.
