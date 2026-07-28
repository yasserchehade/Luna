# Cloud Assistance acceptance coverage

Tests use the approved local-core, boundary-contract and installed-application seams.

| Requirement | Evidence |
| --- | --- |
| Explicit provider and model | `luna_sends_the_explicit_provider_and_model_through_the_gateway_contract` |
| Provider-specific consent | `consent_for_one_intelligence_provider_cannot_authorise_another` |
| Local scope, model and disclosed-field changes invalidate consent | `reusable_consent_cannot_expand_to_new_local_scope_response_fields_or_models` |
| Allow once is not reusable | `allow_once_consent_is_persisted_consumed_and_cannot_be_reused` |
| Failed authentication does not consume allow-once | `missing_gateway_authentication_does_not_consume_one_time_consent` |
| Missing BYOK credential fails before transmission or consent use | `missing_byok_provider_credential_fails_closed_before_transmission_or_consent_use` |
| BYOK-only access does not imply managed entitlement | `a_byok_only_household_is_not_reported_as_having_managed_access` |
| Reusable consent is inspectable/revocable | `reusable_consent_is_inspectable_and_revocable` |
| Keep local sends nothing | `keep_local_records_the_choice_without_transmitting_document_information` and installed Conversation scenario |
| Request minimisation | `allow_once_cloud_assistance_returns_a_validated_candidate_without_filing_the_document` |
| Secret exclusion | `gateway_credentials_remain_in_the_vault_and_out_of_public_history` through the credential-vault boundary |
| BYOK vault-only retention | `byok_connection_is_tested_before_the_provider_key_is_kept_only_in_the_vault` |
| Separate gateway/provider authentication | `byok_adapter_separates_gateway_and_provider_credentials_on_the_isolated_route` and `byok-canary.test.mjs` |
| Installed BYOK setup | installed Conversation scenario tests connect, replace and remove in Options |
| Deterministic contract | all `DeterministicIntelligenceGateway` application and boundary tests |
| Invalid output rejection | `invalid_structured_identity_is_rejected_and_recorded_as_waiting` |
| Untrusted Evidence bounds and provenance | `oversized_or_untraceable_provider_evidence_is_rejected_through_the_gateway_boundary` |
| No provider-owned mutation | the allow-once application test proves the Original remains unfiled |
| Owning-domain validation | `invalid_candidate_amount_is_rejected_into_a_recoverable_waiting_state`, `invalid_candidate_calendar_date_is_rejected_by_document_handling` and `candidate_cannot_replace_context_that_already_has_member_direction` |
| No cross-provider fallback | `safe_retry_reuses_the_exact_provider_model_and_request` |
| Recoverable provider failure | `provider_failure_retries_only_the_selected_route_and_preserves_waiting_work` |
| Local Filing Rules | existing rule-matching and automatic-filing integration tests |
| Duplicate and Document Version | existing exact/Possible Duplicate and updated-version integration tests |
| Replaceable adapter | the same `DocumentIntelligenceService` tests run with the deterministic gateway |
| Safe immutable History | `candidate_disposition_appends_history_without_rewriting_the_presented_event` plus application coverage for rejected candidates |
| Consent conversation | `desktop/e2e/conversation.spec.ts` names OpenAI, explains the exact reusable scope before consent, and exercises allow-once, reusable consent, reuse, revocation, and Keep local |
| Customer-only setup boundary | the installed Options scenario proves managed access status is visible while no password field or manual Luna gateway-credential control is exposed |
| Partial protected-state failure and customer-safe copy | `CloudAssistanceOptions.test.ts` proves provider setup remains available, older unreadable Consent Grants are not reused, implementation details stay hidden and provider-key links remain unambiguous |
| Operational canary runner | `node --test ops/litellm/canary.test.mjs` proves exact model scoping, the strict structured-result contract, LiteLLM route-identity handling, usage evidence, revocation and secret-free output through the public HTTP and CLI seams; `run-local-canary.test.ps1` locks down Windows PowerShell native stderr/exit-code handling |

The OpenAI/LiteLLM canary is an explicit release-environment check, not an automated-suite dependency. For prototype acceptance it may run through the pinned, operator-run loopback deployment using only the fixed synthetic request and disposable credentials. On 28 July 2026 the real OpenAI `gpt-4.1-mini` canary passed through the exact `openai/gpt-4.1-mini` LiteLLM route with a strict structured result and privacy-safe usage of 397 input, 214 output and 611 total tokens. The disposable virtual key was revoked and proved unusable; the synthetic marker, upstream key, master key and database password were absent from container logs; the encrypted handoff, containers, networks and volume were removed. Privacy-safe evidence is attached to issue #13 and PR #33.

A remote host is not required to merge the prototype. Issue #53 owns the separate pre-production gate: authenticated TLS ingress, managed secrets, attributable client credentials, abuse controls and remote log verification must exist before external testers use Luna-managed Intelligence.

Bring-your-own Intelligence is a separate follow-on gate. The pinned LiteLLM build can distinguish Luna gateway authentication from a forwarded provider credential, but Luna must prove BYOK-only route isolation, fail-closed missing-key behavior, credential-free logs and non-persistence before exposing provider-key entry in Options.

Source-boundary review additionally verifies `conversation.rs` and `document_intelligence.rs` import no LiteLLM type. `litellm.rs` is private to the Rust crate.
