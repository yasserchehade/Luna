# Operating the provisional LiteLLM gateway

The desktop does not run LiteLLM or Python. A separately deployed Luna-managed service exposes the OpenAI-compatible chat-completions endpoint configured by `LUNA_MANAGED_INTELLIGENCE_URL`.

## Server configuration

`ops/litellm/config.yaml` is a non-secret baseline. Supply these values through the server secret manager:

- `OPENAI_API_KEY` — upstream credential, never exposed to a desktop;
- `LITELLM_MASTER_KEY` — administrative bootstrap credential;
- `DATABASE_URL` — gateway identity, virtual-key and privacy-safe usage metadata.

Provision a separate narrow LiteLLM virtual key for each attributable Trusted Device or Household. The desktop stores that key through **Options → Cloud assistance** in the operating-system credential vault. Do not distribute the master key to a desktop.

The allowlisted LiteLLM model name is exactly `openai/gpt-4.1-mini`. Do not add an alias shared by multiple deployments, fallbacks, context-window fallbacks or load-balancing deployments under that name.

## Local and CI testing

Standard tests use `DeterministicIntelligenceGateway` or a mock HTTP transport. They do not require LiteLLM, a provider key or a paid call.

For an opt-in integration environment:

1. deploy the pinned LiteLLM proxy with `ops/litellm/config.yaml`;
2. create a disposable virtual key;
3. set `LUNA_MANAGED_INTELLIGENCE_URL` to its `/v1/chat/completions` endpoint;
4. enter the virtual key through the running Luna Options surface;
5. use a synthetic Document with no real Household information;
6. verify request identity, structured result, usage metadata and body-free logs;
7. revoke the key.

Live paid calls are manual evaluation evidence and are not part of the standard suite.

## Upgrade policy

LiteLLM remains provisional. Pin a reviewed stable image digest rather than `main-latest`. Re-run the logging canary, exact-route, structured-output, authentication, timeout and rate-limit checks on every upgrade. Evaluate Portkey and direct Rust adapters before expanding the gateway's role or provider catalogue.
