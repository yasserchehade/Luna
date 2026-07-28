# Operating Household Plans in the prototype

The first prototype accepts no live payments. It supports bounded complimentary beta access and Paddle sandbox sessions so Luna can prove the complete Household Plan lifecycle without charging a Household.

## Server-only configuration

Configure these values as Supabase Edge Function secrets, never in a desktop environment, repository file, Cabinet or local database:

- `PADDLE_API_KEY` — Paddle sandbox API key;
- `PADDLE_WEBHOOK_SECRET` — Paddle notification destination secret;
- `PADDLE_MANAGED_PRICE_ID` — the single accepted sandbox price;
- `PADDLE_API_BASE_URL` — `https://sandbox-api.paddle.com` for the prototype;
- `LITELLM_ADMIN_URL` — remote HTTPS LiteLLM administration endpoint;
- `LITELLM_MASTER_KEY` — LiteLLM administrative key;
- `LUNA_MANAGED_INTELLIGENCE_URL` — customer-facing remote HTTPS chat-completions endpoint;
- `LUNA_MANAGED_REQUEST_LIMIT`, `LUNA_MANAGED_MAX_BUDGET`, `LUNA_MANAGED_RPM_LIMIT` and `LUNA_MANAGED_TPM_LIMIT` — prototype abuse caps;
- `LUNA_RECONCILIATION_SECRET` — independent high-entropy bearer value for scheduled reconciliation calls.

Supabase supplies `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` to the functions. Keep live Paddle credentials disabled until the remote-gateway gate in issue #53 is complete.

## Complimentary beta grant

An operator grants access to a Household, not to an email address or an application build. Look up and verify the intended Household identifier, then invoke `grant_complimentary_managed_intelligence` with the service role. Set a bounded request limit and expiry. Ordinary authenticated members cannot call this function.

After the next unlock, an entitled enrolled Trusted Device requests a five-minute challenge, signs it with its existing device-authorisation key and asks the provisioning function for a narrow 24-hour LiteLLM virtual key. The desktop retains that key only in its operating-system credential vault.

To end the grant, invoke `revoke_complimentary_managed_intelligence` with the service role. The database queues each attributable LiteLLM alias for removal and the desktop clears its local key on its next authenticated synchronisation.

## Paddle sandbox

Set Paddle's default payment link to an HTTPS page on a domain controlled by Luna. The app asks `household-billing-session` for a transaction checkout URL; it never collects card data. Paddle must send subscription events to `paddle-webhook`. Only the configured sandbox price and a signed opaque `household_id` reference are accepted.

The customer portal is also Paddle-hosted and available only after the Household has an external customer and subscription identifier. Sandbox screens must continue to say that no real charge will be made.

## Reconciliation

Invoke both reconciliation functions periodically from a protected scheduler:

1. `reconcile-paddle-subscriptions` fetches the current Paddle state for known subscriptions and sends it through the same idempotent, event-ordered database function as webhooks.
2. `reconcile-managed-intelligence-access` expires ended entitlements and retries each queued LiteLLM alias deletion.

Send `x-luna-reconciliation-secret` and expose neither endpoint publicly without that check. Alert on repeated non-2xx responses. Do not log request authorisation, Paddle payloads, generated virtual keys or the LiteLLM master key.

## Promotion gate

Before any external Household receives managed access, complete issue #53: remote authenticated TLS ingress, managed secrets, working per-device provisioning/revocation, abuse-control evidence and credential/content-free ingress and LiteLLM logs. Then repeat the installed checkout, provisioning, Cloud Assistance, cancellation and revocation journey against sandbox infrastructure before considering live billing.
