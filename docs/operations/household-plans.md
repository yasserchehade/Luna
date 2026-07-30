# Operating Household Plans in the prototype

The first prototype accepts no live payments. It supports bounded complimentary beta access and Paddle sandbox sessions so Luna can prove the complete Household Plan lifecycle without charging a Household.

## Beta deployment status

On 29 July 2026, the linked `luna-beta` project was migrated through `202607280014`. The designated founder Household has an operator-granted complimentary entitlement capped at USD 5 and expiring on 2 September 2026. A separate internal canary Household has a USD 0.25 entitlement expiring on 12 August 2026. Both grants target only server-side Household identifiers; no member identity or allowlist is present in the desktop or public source.

The named Cloudflare Tunnel, separate public and administration hostnames, Service Auth policy, managed gateway secrets and two Supabase administration functions are live. Founder review found that an earlier protected portable-memory recovery failure could prevent the desktop from reaching managed provisioning while reporting the whole coordination pass as offline. The desktop now attempts portable-memory recovery and managed access independently after it verifies Trusted Device authorization, and it names the failing subsystem while preserving local work and automatic retry.

The founder Trusted Device successfully provisioned its narrow gateway credential after the administration URL and rotated Cloudflare Service Auth identifier were corrected in Supabase. The reconciliation endpoint then cleared the failed alias with zero failures, and the desktop reported only the independent portable-memory warning. On 30 July the separate MFA canary completed first-device enrollment, managed provisioning, exact-default selection, Conversation permission and a real `openai/gpt-4.1-mini` reply through the public gateway. That live run found and corrected the packaged desktop's invalid default gateway URL and the Options page's stale provisioning state. Entitlement revocation still requires explicit release-gate evidence before the remote gate closes.

## Server-only configuration

Configure these values as Supabase Edge Function secrets, never in a desktop environment, repository file, Cabinet or local database:

- `PADDLE_API_KEY` — Paddle sandbox API key;
- `PADDLE_WEBHOOK_SECRET` — Paddle notification destination secret;
- `PADDLE_MANAGED_PRICE_ID` — the single accepted sandbox price;
- `PADDLE_MANAGED_MAX_BUDGET_USD` — the Household-level sandbox entitlement budget;
- `LITELLM_ADMIN_URL` — remote HTTPS LiteLLM administration endpoint;
- `LITELLM_MASTER_KEY` — LiteLLM administrative key;
- `CLOUDFLARE_ACCESS_CLIENT_ID` and `CLOUDFLARE_ACCESS_CLIENT_SECRET` — revocable Service Auth credentials for the protected administration hostname;
- `LUNA_MANAGED_INTELLIGENCE_URL` — customer-facing remote HTTPS chat-completions endpoint;
- `LITELLM_DEVICE_KEY_DURATION_HOURS` — virtual-key lifetime, defaulting to 24 hours and rejected unless it is an integer from 2 through 24;
- `LITELLM_ADMIN_REQUEST_TIMEOUT_MS` — each LiteLLM administration call, defaulting to 10 seconds and rejected above 15 seconds so the complete mint path remains well inside the two-minute provisioning lease;
- `LITELLM_HOUSEHOLD_RPM_LIMIT` and `LITELLM_HOUSEHOLD_TPM_LIMIT` — shared Household throughput caps;
- `LUNA_RECONCILIATION_SECRET` — independent high-entropy bearer value for scheduled reconciliation calls.

Supabase supplies `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` to the functions. Keep live Paddle credentials disabled until the remote-gateway gate in issue #53 is complete.

## Complimentary beta grant

An operator grants access to a Household, not to an email address or an application build. Look up and verify the intended Household identifier, then invoke `grant_complimentary_managed_intelligence` with the service role. Set a bounded USD budget and expiry. Ordinary authenticated members cannot call this function. The generated entitlement budget scope becomes one shared LiteLLM team budget across all of that Household's device keys.

Keep the founder Household separate from the internal canary Household. The canary account is a normal Household Organiser for only its own Household, not a platform administrator. Provision or rotate it with `ops/litellm/provision-managed-canary.mjs`; the command requires an authenticated Supabase CLI, an exact match with the repository's linked project and explicit `--confirm-live`. It grants a small expiring complimentary budget through the existing operator-only function, resets its login and authenticator, and writes the login plus authenticator seed only to a caller-selected Windows DPAPI-protected file outside the repository. The single-use bootstrap SQL file exists only inside a verified Windows Temp directory and is removed immediately after the linked query. Never put that credential file, an account password or an authenticator seed in source control, CI output or an issue comment.

The persistent canary supports repeatable account, Trusted Device, managed-key, provider-request, expiry and revocation checks without signing the founder's review application out of its Household. Do not use the canary to bypass deterministic installed-app test doubles: routine CI stays provider-free and a live canary is an explicitly operated pre-release check.

After the next unlock, an entitled enrolled Trusted Device requests a five-minute challenge, signs it with its existing device-authorisation key and asks the provisioning function for a narrow LiteLLM virtual key whose requested and returned lifetime cannot exceed 24 hours. Before minting, the server reserves the deterministic device alias while rechecking that the Household entitlement, Trusted Device, budget scope and cap still match the authorization. A short provisioning lease keeps reconciliation from completing alias deletion before the in-flight mint finishes. The desktop retains the returned key only in its operating-system credential vault.

The packaged internal-beta desktop defaults to `https://intelligence-beta.silikin.com/v1/chat/completions`; this customer endpoint is public configuration, not a credential. `LUNA_MANAGED_INTELLIGENCE_URL` remains an operator override. Do not require members to launch Luna from a configured shell.

To end the grant, invoke `revoke_complimentary_managed_intelligence` with the service role. The database queues each attributable LiteLLM alias for removal and the desktop clears its local key on its next authenticated synchronisation.

## Paddle sandbox

Set Paddle's default payment link to an HTTPS page on a domain controlled by Luna. The app asks `household-billing-session` for a transaction checkout URL; it never collects card data. Paddle must send subscription events to `paddle-webhook`. Only the configured sandbox price and a signed opaque `household_id` reference are accepted. The billing adapter is compiled against `https://sandbox-api.paddle.com`; no environment setting can redirect this prototype code to Paddle's live API.

The customer portal is also Paddle-hosted and available only after the Household has an external customer and subscription identifier. Sandbox screens must continue to say that no real charge will be made.

## Reconciliation

Invoke both reconciliation functions periodically from a protected scheduler:

1. `reconcile-paddle-subscriptions` fetches the current Paddle state for known subscriptions and sends it through the same idempotent, event-ordered database function as webhooks.
2. `reconcile-managed-intelligence-access` expires ended entitlements and retries each queued LiteLLM alias deletion.

Send `x-luna-reconciliation-secret` and expose neither endpoint publicly without that check. Alert on repeated non-2xx responses. Do not log request authorisation, Paddle payloads, generated virtual keys or the LiteLLM master key.

## Promotion gate

Before any external Household receives managed access, complete issue #53: remote authenticated TLS ingress, managed secrets, working per-device provisioning/revocation, abuse-control evidence and credential/content-free ingress and LiteLLM logs. Then repeat the installed checkout, provisioning, Cloud Assistance, cancellation and revocation journey against sandbox infrastructure before considering live billing.
