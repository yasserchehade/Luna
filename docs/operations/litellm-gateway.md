# Operating the provisional LiteLLM gateway

The desktop does not run LiteLLM or Python. A separately operated Luna-managed service exposes the OpenAI-compatible chat-completions endpoint configured by `LUNA_MANAGED_INTELLIGENCE_URL`. The pinned Compose deployment supports an ephemeral operator-run prototype canary on loopback and a later remote pre-production deployment; neither is a desktop sidecar.

## Server configuration

`ops/litellm/config.yaml` and `ops/litellm/byok-config.yaml` are non-secret baselines. The managed service receives `OPENAI_API_KEY`; the separate BYOK service must never receive that variable or another Luna-funded provider credential. Supply operator values through the local prototype environment and production values through the server secret manager:

- `OPENAI_API_KEY` — upstream credential, never exposed to a desktop;
- `LITELLM_MASTER_KEY` — administrative bootstrap credential;
- `DATABASE_URL` — gateway identity, virtual-key and privacy-safe usage metadata.

Provision a separate narrow LiteLLM virtual key for each attributable Trusted Device or Household. The desktop stores that automatically provisioned key in the operating-system credential vault. Do not expose virtual-key entry as customer setup, and never distribute the master key to a desktop.

The managed allowlisted model is exactly `openai/gpt-4.1-mini`; the BYOK-only model is exactly `byok/openai/gpt-4.1-mini`. Do not add an alias shared by multiple deployments, fallbacks, context-window fallbacks or load-balancing deployments under either name.

## Pinned gateway deployment

`ops/litellm/compose.yaml` is the reproducible single-host staging deployment. The selected LiteLLM build is the upstream [v1.93.0 stable release](https://github.com/BerriAI/litellm/releases/tag/v1.93.0). It pins:

- LiteLLM database image `v1.93.0` at multi-platform digest `sha256:72360d8bd5602faa49be5098a8ac3dd069d9fb74503d6bd014242d96dc753e43`;
- PostgreSQL 16 at multi-platform digest `sha256:fe03a7605299a34ddf5e4f285dff78c3d7190a576b3c6b46f2fcff69f4bffd54`.

Verify the LiteLLM image signature against the signing key introduced by upstream commit `0112e53` before deployment:

```powershell
cosign verify `
  --key https://raw.githubusercontent.com/BerriAI/litellm/0112e53046018d726492c814b3644b7d376029d0/cosign.pub `
  ghcr.io/berriai/litellm-database:v1.93.0@sha256:72360d8bd5602faa49be5098a8ac3dd069d9fb74503d6bd014242d96dc753e43
```

Inject `OPENAI_API_KEY`, `LITELLM_MASTER_KEY`, `LITELLM_DATABASE_PASSWORD` and `DATABASE_URL` into the Compose process. For the prototype canary, load them into the operator shell from an approved local secret store; for remote deployment, use the staging secret manager. Do not create a repository or server `.env` file containing them. `DATABASE_URL` must address the Compose service name `database` and use the same URL-encoded password supplied as `LITELLM_DATABASE_PASSWORD`.

```powershell
docker compose -f ops/litellm/compose.yaml config --quiet
docker compose -f ops/litellm/compose.yaml up -d
docker compose -f ops/litellm/compose.yaml ps
```

The managed and BYOK gateways bind only to `127.0.0.1` ports 4000 and 4001 by default. Keep those bindings for the operator-run prototype canary. A remote staging host must put authenticated TLS ingress in front of them; ingress must omit request bodies and authorisation headers from access and error logs. Do not publish either port directly.

## No-host-cost beta ingress

ADR 0018 selects a named Cloudflare Tunnel on the Free plan as the internal-beta bridge. It uses the operator-controlled Windows prototype machine, so it has no separate monthly host charge and is available only while that machine, Docker Desktop and the tunnel are running. It is not production hosting.

Cloudflare must be authoritative for the beta domain on the Free plan. Review existing DNS records before changing nameservers and handle existing DNSSEC exactly as the registrar and Cloudflare onboarding screens require.

In Cloudflare Zero Trust:

1. create a remotely managed tunnel named `luna-beta-gateway`;
2. add `intelligence-beta.<domain>` with service `http://managed-public-ingress:8080`;
3. add `intelligence-admin-beta.<domain>` with service `http://managed-admin-ingress:8081`;
4. create a self-hosted Access application for the complete administration hostname;
5. create a Service Auth policy that includes only a service token named `luna-supabase-beta`; do not add an Everyone, Allow or Bypass policy;
6. retain the service-token client ID and one-time secret only in the Supabase Edge Function secret store.

The customer hostname is protected by the Trusted Device's narrow LiteLLM bearer key and the local public ingress forwards only `/v1/chat/completions` and `/v1/models`. The administration hostname is a separate defence-in-depth boundary: Cloudflare Access authenticates the Supabase service token, the local ingress forwards only the allowlisted team/key/health routes, and LiteLLM still requires the master key.

The pre-production overlay pins Cloudflare Tunnel `2026.7.2` and Caddy `2.10.2` by multi-platform digest. Caddy access logging is not enabled. Both Caddy services use a read-only root filesystem, `no-new-privileges`, `cap_drop: ALL` and then restore only `NET_BIND_SERVICE`, which the official Caddy binary carries as a file capability and therefore requires even though Luna's listeners use unprivileged ports. The tunnel token is read from a file outside the repository rather than appearing in the container command. Point `LUNA_CLOUDFLARE_TUNNEL_TOKEN_FILE` at the operator-secret-store handoff, then render and start both Compose files:

```powershell
docker compose `
  -f ops/litellm/compose.yaml `
  -f ops/litellm/compose.cloudflare.yaml `
  config --quiet

docker compose `
  -f ops/litellm/compose.yaml `
  -f ops/litellm/compose.cloudflare.yaml `
  up -d
```

Do not create a repository `.env` file. The OpenAI key, LiteLLM master key, database password, database URL and tunnel-token file must be injected from the operator's protected local secret store. The tunnel token handoff must be readable only by the operator and removed when the deployment is retired.

Configure the Supabase function secret store with:

- `LUNA_MANAGED_INTELLIGENCE_URL=https://intelligence-beta.<domain>/v1/chat/completions`;
- `LITELLM_ADMIN_URL=https://intelligence-admin-beta.<domain>`;
- `CLOUDFLARE_ACCESS_CLIENT_ID` and `CLOUDFLARE_ACCESS_CLIENT_SECRET`;
- the existing LiteLLM master, duration, request-limit and reconciliation values.

The two administration functions attach the Cloudflare service-token headers only to LiteLLM administration calls. The desktop and customer chat-completions requests never receive them.

### Internal-beta deployment evidence

On 29 July 2026, `silikin.com` became the authoritative internal-beta domain for
the healthy `luna-beta-gateway` tunnel. The public route is
`https://intelligence-beta.silikin.com` and the Access-protected administration
route is `https://intelligence-admin-beta.silikin.com`. The administration
application has one Service Auth policy whose only include rule is the
`luna-supabase-beta` service token; unauthenticated administration receives HTTP
403. Public `/v1/models` requires a LiteLLM key and the public ingress returns 404
for the administration health path.

The `luna-beta` Supabase secret store contains the two endpoint URLs, Cloudflare
service-token credentials, the LiteLLM master key, reconciliation secret and
bounded duration, timeout, RPM and TPM settings. Both
`managed-intelligence-provisioning` and
`reconcile-managed-intelligence-access` are active. The complete pinned Docker
stack is healthy on the operator machine.

A bounded synthetic `gpt-4.1-mini` call passed through the running managed
gateway using the exact `openai/gpt-4.1-mini` route and strict structured-result
contract. It used 399 input, 216 output and 615 total tokens; its disposable
virtual key was revoked. A content-blind scan of all six container logs found
neither `LUNA_SYNTHETIC_CANARY_53` nor any checked provider, gateway, database or
tunnel credential.

On 30 July the isolated installed desktop canary also completed the member
journey through this hostname: MFA sign-in, first Trusted Device enrollment,
narrow credential provisioning, exact-default selection, Conversation
permission, Enter submission, real reply and a completed exact-route audit
event. A gateway-offline provisioning attempt first created a durable failed
alias; the protected reconciler removed one alias with zero failures before the
successful retry. The desktop now packages this non-secret public customer URL
as its beta default instead of requiring a process environment variable.

This is internal-beta infrastructure evidence, not production readiness. Issue
#53 remains open until the designated Trusted Device provisions through the
deployed Edge Function, completes a managed request through the public hostname
and proves entitlement/key revocation.

## Local and CI testing

Standard tests use `DeterministicIntelligenceGateway` or a mock HTTP transport. They do not require LiteLLM, a provider key or a paid call.

For the opt-in prototype canary:

1. place the OpenAI key in the user-local DPAPI handoff described below;
2. run `ops/litellm/run-local-canary.ps1` from the repository root;
3. let the wrapper generate disposable gateway/database secrets, deploy both pinned proxies on loopback and run the managed plus BYOK canaries;
4. inspect LiteLLM, error and spend logs for the fixed marker `LUNA_SYNTHETIC_CANARY_53` and prove it is absent;
5. attach the runner's privacy-safe JSON result plus redacted log and deployment evidence to issue #13 and PR #33;
6. confirm the wrapper removed the encrypted handoff, test credentials, containers, networks and volume.

Create the encrypted handoff without placing the key in shell history:

```powershell
$lunaSecretDir = Join-Path $env:LOCALAPPDATA 'Luna'
New-Item -ItemType Directory -Force -Path $lunaSecretDir | Out-Null
Read-Host 'OpenAI API key' -AsSecureString |
  Export-Clixml -LiteralPath (Join-Path $lunaSecretDir 'openai-canary-key.clixml')
```

Then run:

```powershell
& .\ops\litellm\run-local-canary.ps1
```

On Windows, the handoff is encrypted for the current user. The wrapper refuses pre-existing `luna-litellm` Docker resources, binds the gateway to loopback, redacts failures, scans logs for the synthetic marker and all generated credentials, tears down the Compose project and deletes the encrypted handoff. It writes only privacy-safe evidence to `%LOCALAPPDATA%\Luna\litellm-canary-result.json`.

The managed runner creates a 15-minute, USD 0.10 maximum disposable virtual key restricted to `openai/gpt-4.1-mini` and the `/v1/models` plus `/v1/chat/completions` routes, sends the same strict structured-output contract as the desktop adapter, checks the gateway-reported approved route identity and token usage metadata, revokes the key and proves the revoked key is rejected. It emits neither the master key, virtual key nor synthetic document text. On a failed completion it still attempts to revoke the disposable key.

Pinned LiteLLM v1.93.0 deliberately reports the requested proxy route in `response.model`, replacing OpenAI's downstream model string. The canary therefore accepts the exact `openai/gpt-4.1-mini` route as well as OpenAI's base or dated `gpt-4.1-mini` identities. Route proof is joint: the pinned configuration maps only that route, the disposable key exposes only that model, the request names it exactly, and retries/fallbacks remain disabled.

The BYOK canary creates a separate disposable virtual key restricted to `byok/openai/gpt-4.1-mini`, sends gateway and customer-provider authentication in separate headers, proves a missing customer key fails, proves the same key receives HTTP 403 for the managed route, revokes the virtual key, and scans both container logs and a PostgreSQL dump for the customer key and synthetic marker. The rendered BYOK container environment has no managed provider secret.

The runner and desktop adapter accept HTTP only for loopback. Every remote endpoint must use HTTPS and is rejected before credentials or request content are transmitted. The desktop reads at most 8 KiB of a failed response and retains only the structured error type needed to distinguish a rejected virtual key from a rejected upstream provider key. Its deterministic boundary tests run with:

```powershell
node --test ops/litellm/canary.test.mjs ops/litellm/byok-canary.test.mjs
node --test ops/litellm/compose.test.mjs
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ops/litellm/run-local-canary.test.ps1
```

For the remote canary, additionally set `LITELLM_ADMIN_URL`, `CLOUDFLARE_ACCESS_CLIENT_ID` and `CLOUDFLARE_ACCESS_CLIENT_SECRET`. `canary.mjs` refuses a non-loopback deployment unless customer traffic and protected administration have separate endpoints. Live paid calls are manual evaluation evidence and are not part of the standard suite.

Issue #53 owns the remote pre-production deployment and its authenticated TLS ingress, managed secrets, attributable client credentials, abuse controls and ingress-log verification. That deployment is required before external testing, not before the prototype contract can be accepted.

Household Plan provisioning and revocation are operated through the account service described in `docs/operations/household-plans.md`. The LiteLLM master key is consumed only by the server-side provisioning and revocation functions; it must never be configured in a desktop build.

## Upgrade policy

LiteLLM remains provisional. Pin a reviewed stable image digest rather than `main-latest`. Re-run the logging canary, exact-route, structured-output, authentication, timeout and rate-limit checks on every upgrade. Evaluate Portkey and direct Rust adapters before expanding the gateway's role or provider catalogue.
