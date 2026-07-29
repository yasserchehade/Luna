# ADR 0018: Use a Cloudflare Tunnel for the no-host-cost beta

## Decision

Luna's first remotely reachable Managed Intelligence gateway will run from the operator-controlled Windows prototype machine through a named Cloudflare Tunnel on the Free plan. This is an internal-beta bridge, not the production hosting architecture.

The deployment uses the digest-pinned `ops/litellm/compose.yaml` services plus `ops/litellm/compose.cloudflare.yaml`. The latter adds:

- a digest-pinned Cloudflare Tunnel connector with no inbound host port;
- a customer ingress that forwards only `/v1/chat/completions` and `/v1/models`;
- a separate administration ingress that forwards only the LiteLLM health, team and key-management routes;
- a Cloudflare Access Service Auth policy on the administration hostname.

The public customer hostname is authenticated by each Trusted Device's narrow, attributable LiteLLM virtual key. The administration hostname additionally requires a revocable Cloudflare Access service token held only by the Supabase functions, and LiteLLM still requires its master key. No desktop receives either administrative credential.

The base LiteLLM ports remain bound to `127.0.0.1`. Cloudflare terminates public TLS and `cloudflared` initiates outbound-only connectivity, so the operator does not open router or firewall ingress ports.

## Why

The prototype needs a real remotely reachable gateway before Managed Intelligence can be exercised end to end, but it does not yet justify a continuously billed application host. Cloudflare's Free Zero Trust plan supports a proof-of-concept of this size, and the Household already owns a domain that can provide stable beta hostnames.

This choice avoids a monthly hosting commitment. It does not avoid OpenAI usage, domain-registration or future production-hosting costs.

## Consequences

- Managed Intelligence is unavailable whenever the operator machine, Docker Desktop or tunnel is stopped.
- The beta has no paid uptime SLA and is unsuitable for production or a general public launch.
- Cloudflare becomes the provisional DNS/TLS/ingress dependency, while LiteLLM remains replaceable behind Luna's provider-neutral contract.
- The tunnel token and gateway/provider secrets must come from the operator secret store and must never be committed, posted to GitHub or embedded in the desktop.
- A later production decision must move the same pinned deployment to an always-on managed host, add monitoring and backups, and repeat canary, revocation and log-privacy evidence before external launch.

## Promotion gate

Issue #53 remains open until the named tunnel, two hostnames, Access service token, Supabase secrets and running gateway are configured; the synthetic real-provider canary passes through the public hostname; per-device provisioning and revocation pass; and redacted logs prove that credentials and the synthetic marker were not recorded.
