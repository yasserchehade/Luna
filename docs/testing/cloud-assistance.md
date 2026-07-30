# Cloud Assistance acceptance coverage

Tests use the approved local-core, boundary-contract and installed-application seams.

| Requirement | Evidence |
| --- | --- |
| Explicit provider and model | `luna_sends_the_explicit_provider_and_model_through_the_gateway_contract` |
| Current-message-only Conversation disclosure | `conversation_reply_sends_only_the_new_message_through_the_selected_route` |
| Conversation reply has no tools, authority or model-generated identity | `conversation_adapter_accepts_a_bounded_reply_without_model_generated_identity` |
| Default Document permission spans Documents without per-Document consent | `default_document_permission_applies_without_per_document_scope_matching` |
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
| Provider setup waits for Luna access | `a_byok_only_household_is_not_reported_as_having_managed_access` plus the installed Options scenario, which remains on Cloud Assistance while delayed managed provisioning changes from **Preparing this Trusted Device** to **Managed access ready** through the polling path |
| Cleartext endpoint rejection | `cleartext_gateway_endpoints_are_allowed_only_on_loopback` |
| Gateway/provider auth failure attribution | `byok_gateway_authentication_rejection_is_not_blamed_on_the_provider_key` |
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
| Default-permission conversation | `desktop/e2e/conversation.spec.ts` selects the exact route, enumerates the Document disclosure before permission, exercises independent permission use and revocation, proves typed “Allow once” cannot bypass a disabled Document permission, and retains Keep local |
| Customer-only setup boundary | the installed Options scenario proves managed access status is visible while no password field or manual Luna gateway-credential control is exposed |
| Partial protected-state failure | `verified_consent_lookup_skips_unreadable_history_without_reusing_it` proves unreadable Consent history is excluded from authorization and portable export while verified current grants remain available; founder installed-app review records the warning beside usable provider controls |
| Provider setup copy | the installed Options scenario verifies the unambiguous **OpenAI API keys** link and connect/replace/remove flow through the public application seam |
| Independent post-trust recovery | `managed access still synchronizes when portable memory recovery fails` proves unreadable protected portable memory cannot prevent an eligible Trusted Device from attempting managed credential provisioning |
| Accurate managed-access failure copy | `managed provisioning failures are not mislabeled as a general offline state` keeps local availability distinct from Cloud Assistance preparation and preserves automatic retry |
| Offline shell geometry | the installed account E2E measures the real sidebar and Conversation bounds while coordination is unavailable, proving notices cannot auto-place the workspace beneath the sidebar |
| Cabinet overflow | the installed Conversation E2E proves the Cabinet surface and every filed-Original card retain zero horizontal overflow for long destinations and checksums |
| Operational canary runner | `node --test ops/litellm/canary.test.mjs` proves exact model scoping, the strict structured-result contract, LiteLLM route-identity handling, usage evidence, revocation, secret-free output and separation of customer traffic from Cloudflare Access-protected administration through the public HTTP and CLI seams; `run-local-canary.test.ps1` locks down Windows PowerShell native stderr/exit-code handling |
| Persistent managed canary identity | `ops/litellm/provision-managed-canary.mjs` creates or rotates a normal MFA-protected Household Organiser, grants only its Household a small expiring complimentary budget through the operator function, and stores the login plus authenticator seed in a caller-selected Windows DPAPI file outside the repository |
| Household entitlement boundary | `desktop/account-contract/supabaseAccountService.test.ts` proves operator-only complimentary grants, ordered/idempotent Paddle state, short-lived Trusted Device proof, one-use challenge consumption and entitlement revocation |
| Billing server boundary | `desktop/server-contract/householdBillingSession.test.ts` and `paddleWebhook.test.ts` prove authenticated checkout/portal sessions, minimal Paddle checkout data, raw-body signature rejection and exact-price enforcement |
| Managed device provisioning | `desktop/server-contract/managedIntelligenceProvisioning.test.ts` proves device proof reaches only narrow credential generation, rejects configuration and returned expiries above 24 hours, revokes an unsafe generated key, bounds gateway calls inside the reservation lease, records failed mints for durable cleanup, stops minting after concurrent entitlement loss, and proves exact route/model, expiry, shared Household budget, rate/token caps, opaque attribution and alias revocation. The account contract proves stale budget authorization and stale readiness are rejected, an in-flight alias cannot be cleared prematurely and an expired reservation remains queued for deletion. |
| Missed-event recovery | `desktop/server-contract/paddleReconciliation.test.ts` proves reconciliation reuses the ordered Paddle event seam |
| Installed Household Plan states | `desktop/e2e/conversation.spec.ts` proves the free checkout prompt, delayed provisioning poll and complimentary-ready state in the rebuilt desktop |
| MFA-verified first device | `desktop/e2e/account.spec.ts` signs in with verified MFA and no remote Trusted Devices, observes the first-device path rather than recovery or authenticator re-enrolment, and completes Recovery Key and PIN setup |
| Installed default-route experience | `desktop/e2e/conversation.spec.ts` selects the exact managed default, enables separate Conversation and Document permissions, proves Enter produces a deterministic current-message-only reply without a per-message prompt, and proves a Document Review Card uses that default without provider/model or per-Document consent selectors |
| Stale default recovery | the installed Options scenario saves an exact BYOK default through the public command seam, removes that connection, observes the configured managed replacement, and requires the Household Organiser to choose **Save default**; permissions do not transfer |

The OpenAI/LiteLLM canary is an explicit release-environment check, not an automated-suite dependency. For prototype acceptance it may run through the pinned, operator-run loopback deployment using only the fixed synthetic request and disposable credentials. On 28 July 2026 the real OpenAI `gpt-4.1-mini` canary passed through the exact `openai/gpt-4.1-mini` LiteLLM route with a strict structured result and privacy-safe usage of 397 input, 214 output and 611 total tokens. The disposable virtual key was revoked and proved unusable; the synthetic marker, upstream key, master key and database password were absent from container logs; the encrypted handoff, containers, networks and volume were removed. Privacy-safe evidence is attached to issue #13 and PR #33.

On 29 July 2026, the persistent internal-beta gateway repeated that contract with
the newly created Luna-funded project key: 399 input, 216 output and 615 total
tokens. The disposable key was revoked, and a content-blind scan of the gateway,
BYOK gateway, database, both Caddy ingress containers and cloudflared found no
synthetic marker or checked credential. The public hostname separately proved
key enforcement and route restriction, while the administration hostname
returned Cloudflare Access HTTP 403 without its Supabase-held service token.

Founder review of the first live-enabled desktop on 29 July exposed two
presentation regressions and one coordination coupling. An offline status node
was a separate child of the two-column shell grid, which auto-placed
Conversation and To do beneath the sidebar; filed-Original flex rows also let
long destinations and checksums expand Cabinet horizontally. The repaired
installed-app checks measure both failures directly. The same review showed
that portable-memory recovery and managed-access provisioning shared one
sequential failure boundary. They now settle independently after Trusted
Device authorization, so a protected-memory recovery failure remains visible
and retryable without suppressing managed provisioning. After the protected
administration URL and rotated Cloudflare Service Auth identifier were
corrected, the founder Trusted Device provisioned its narrow gateway
credential and the revocation reconciler cleared the failed alias with zero
failures. The app then reported only the independent portable-memory warning.
The separate `Luna Managed Canary` account is verified with authenticator MFA,
has no platform-admin flag, and holds a USD 0.25 complimentary entitlement
through 12 August 2026. On 30 July a one-off operator release-environment run enrolled
its first Trusted Device, provisioned a narrow managed credential, selected the
exact `openai/gpt-4.1-mini` default, recorded Conversation permission, received
a real OpenAI reply and verified the completed exact-route audit event. The run
also exposed two release blockers that mocked tests had missed: a failed alias
needed reconciliation after the operator gateway restarted, and packaged
desktop launches fell back to an invalid gateway hostname. The beta endpoint is
now the non-secret packaged default, while the runtime override remains
available for operators. The rebuilt founder application separately displayed
Managed access ready with the same exact default and enabled Conversation
permission, then returned a nonempty reply to a synthetic message submitted by
Enter. The independent protected-memory and Cabinet recovery warnings remained
visible without blocking Cloud Assistance. Credential revocation still requires evidence before
issue #53 can close. That live harness is not part of PR #61; routine installed-app
tests use the deterministic Intelligence boundary required by issue #38.

A remote host is not required to merge the prototype. Issue #53 owns the separate pre-production gate. ADR 0018 selects a no-separate-host-cost named Cloudflare Tunnel from the operator-controlled prototype machine for the internal beta; the machine must be running, and authenticated TLS ingress, managed secrets, attributable client credentials, abuse controls and remote log verification must exist before external testers use Luna-managed Intelligence.

The standard branch checks run the Rust suite, managed-access coordinator boundary tests, server-contract tests, TypeScript checking, Supabase schema lint and account-service contract. Paddle is sandbox-only and all server-contract and installed-app tests use deterministic transports; no real charge or paid provider call is made.

Bring-your-own Intelligence is a separate follow-on gate. Provider-key entry in Options is enabled only after Luna has provisioned BYOK gateway access to the Trusted Device. The pinned LiteLLM build and Luna adapter distinguish Luna gateway authentication from a forwarded provider credential, reject cleartext remote endpoints, and prove BYOK-only route isolation, fail-closed missing-key behavior, credential-free logs and non-persistence.

Source-boundary review additionally verifies `conversation.rs` and `document_intelligence.rs` import no LiteLLM type. `litellm.rs` is private to the Rust crate.
