# Operating the provisional LiteLLM gateway

The desktop does not run LiteLLM or Python. A separately operated Luna-managed service exposes the OpenAI-compatible chat-completions endpoint configured by `LUNA_MANAGED_INTELLIGENCE_URL`. The pinned Compose deployment supports an ephemeral operator-run prototype canary on loopback and a later remote pre-production deployment; neither is a desktop sidecar.

## Server configuration

`ops/litellm/config.yaml` is a non-secret baseline. Supply these values through the operator environment for the local prototype canary and through the server secret manager for remote deployment:

- `OPENAI_API_KEY` — upstream credential, never exposed to a desktop;
- `LITELLM_MASTER_KEY` — administrative bootstrap credential;
- `DATABASE_URL` — gateway identity, virtual-key and privacy-safe usage metadata.

Provision a separate narrow LiteLLM virtual key for each attributable Trusted Device or Household. The desktop stores that key through **Options → Cloud assistance** in the operating-system credential vault. Do not distribute the master key to a desktop.

The allowlisted LiteLLM model name is exactly `openai/gpt-4.1-mini`. Do not add an alias shared by multiple deployments, fallbacks, context-window fallbacks or load-balancing deployments under that name.

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

The gateway binds only to `127.0.0.1` by default. Keep that binding for the operator-run prototype canary. A remote staging host must put an authenticated TLS ingress in front of it; the ingress must omit request bodies and authorisation headers from access and error logs. Do not publish port 4000 directly.

## Local and CI testing

Standard tests use `DeterministicIntelligenceGateway` or a mock HTTP transport. They do not require LiteLLM, a provider key or a paid call.

For the opt-in prototype canary:

1. deploy the pinned LiteLLM proxy with `ops/litellm/compose.yaml`;
2. set `LUNA_MANAGED_INTELLIGENCE_URL` to `http://127.0.0.1:4000/v1/chat/completions`;
3. expose `LITELLM_MASTER_KEY` to the operator shell from the approved local secret store;
4. run `node ops/litellm/canary.mjs`;
5. inspect LiteLLM, error and spend logs for the fixed marker `LUNA_SYNTHETIC_CANARY_53` and prove it is absent;
6. attach the runner's privacy-safe JSON result plus redacted log and deployment evidence to issue #13 and PR #33;
7. remove the test request, revoke all test credentials and tear down the temporary deployment.

The runner creates a 15-minute, USD 0.10 maximum disposable virtual key restricted to `openai/gpt-4.1-mini` and the `/v1/models` plus `/v1/chat/completions` routes, sends the same strict structured-output contract as the desktop adapter, checks the upstream-reported model identity and token usage metadata, revokes the key and proves the revoked key is rejected. It emits neither the master key, virtual key nor synthetic document text. On a failed completion it still attempts to revoke the disposable key.

The runner accepts HTTP only for loopback. Every remote endpoint must use HTTPS. Its deterministic boundary tests run with:

```powershell
node --test ops/litellm/canary.test.mjs
```

Live paid calls are manual evaluation evidence and are not part of the standard suite.

Issue #53 owns the remote pre-production deployment and its authenticated TLS ingress, managed secrets, attributable client credentials, abuse controls and ingress-log verification. That deployment is required before external testing, not before the prototype contract can be accepted.

## Upgrade policy

LiteLLM remains provisional. Pin a reviewed stable image digest rather than `main-latest`. Re-run the logging canary, exact-route, structured-output, authentication, timeout and rate-limit checks on every upgrade. Evaluate Portkey and direct Rust adapters before expanding the gateway's role or provider catalogue.
