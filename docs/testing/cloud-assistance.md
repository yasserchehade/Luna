# Cloud Assistance acceptance coverage

Tests use the approved local-core, boundary-contract and installed-application seams.

| Requirement | Evidence |
| --- | --- |
| Explicit provider and model | `luna_sends_the_explicit_provider_and_model_through_the_gateway_contract` |
| Provider-specific consent | `consent_for_one_intelligence_provider_cannot_authorise_another` |
| Model and disclosed-field changes invalidate consent | `reusable_consent_cannot_expand_to_new_response_fields_or_models` |
| Allow once is not reusable | `allow_once_consent_is_persisted_consumed_and_cannot_be_reused` |
| Failed authentication does not consume allow-once | `missing_gateway_authentication_does_not_consume_one_time_consent` |
| Reusable consent is inspectable/revocable | `reusable_consent_is_inspectable_and_revocable` |
| Keep local sends nothing | `keep_local_records_the_choice_without_transmitting_document_information` and installed Conversation scenario |
| Request minimisation | `allow_once_cloud_assistance_returns_a_validated_candidate_without_filing_the_document` |
| Secret exclusion | `gateway_credentials_never_enter_history_or_ordinary_configuration` |
| Deterministic contract | all `DeterministicIntelligenceGateway` application and boundary tests |
| Invalid output rejection | `invalid_structured_identity_is_rejected_and_recorded_as_waiting` |
| Untrusted Evidence bounds and provenance | `oversized_or_untraceable_provider_evidence_is_rejected` |
| No provider-owned mutation | the allow-once application test proves the Original remains unfiled |
| Owning-domain validation | `invalid_candidate_direction_is_rejected_into_a_recoverable_waiting_state` |
| No cross-provider fallback | `safe_retry_reuses_the_exact_provider_model_and_request` |
| Recoverable provider failure | `provider_failure_retries_only_the_selected_route_and_preserves_waiting_work` |
| Local Filing Rules | existing rule-matching and automatic-filing integration tests |
| Duplicate and Document Version | existing exact/Possible Duplicate and updated-version integration tests |
| Replaceable adapter | the same `DocumentIntelligenceService` tests run with the deterministic gateway |
| Safe History | intelligence tests assert provider, model, Consent Grant, outcome and secret-free reasons |
| Consent conversation | `desktop/e2e/conversation.spec.ts` names OpenAI, explains disclosure, and exercises allow-once, reusable consent, reuse, revocation, and Keep local |

The remote OpenAI/LiteLLM canary is an explicit release-environment check, not an automated-suite dependency. A passing canary must be attached to issue #13 before the evaluated-real-provider criterion is marked complete; this repository/workspace currently contains no deployed gateway endpoint or credential.

Source-boundary review additionally verifies `conversation.rs` and `document_intelligence.rs` import no LiteLLM type. `litellm.rs` is private to the Rust crate.
