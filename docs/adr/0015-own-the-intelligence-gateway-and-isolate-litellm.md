---
status: accepted
---

# Own the Intelligence Gateway and isolate LiteLLM

Luna will own a provider-neutral `IntelligenceGateway` contract and use a remotely operated LiteLLM proxy as the provisional first Luna-managed implementation. Document Handling and Conversation depend only on Luna-owned request, result, failure, Consent Grant and Direction Interpretation types. LiteLLM translation, HTTP details and error mapping remain inside one private Rust adapter.

## Context

Cloud Assistance must support more than one Intelligence Provider without allowing an external harness to own Household authority, privacy policy, provider selection, durable state or consequential actions. The first implementation also must keep Luna's upstream provider credentials out of the desktop, avoid making Python a desktop-wide dependency and remain testable without paid provider calls.

## Decision

The first evaluated Luna-managed route is OpenAI `gpt-4.1-mini` through a Luna-managed remote LiteLLM gateway. Luna selects that exact Intelligence Provider and model, validates a provider-and-model-specific Consent Grant, constructs a bounded structured request, and only then calls the gateway. The adapter sends `openai/gpt-4.1-mini` explicitly and disables LiteLLM retries and fallbacks. Luna may perform a bounded safe retry using the unchanged request, provider and model.

The desktop authenticates with a narrow, revocable Luna gateway credential held in the operating-system credential vault. The remote service holds the upstream OpenAI credential. Neither credential enters SQLite, Cabinet files, History, request content or diagnostics. Production gateway keys are attributable to a Trusted Device or Household for abuse controls and may be independently revoked.

LiteLLM output is untrusted. Luna verifies correlation, provider, model, allowed fields, value constraints and the structured response before converting it to Evidence or a candidate Direction Interpretation. The owning Document Handling domain validates that candidate again, including that the field still awaits Member Direction and that monetary and ISO-date values satisfy domain constraints. No result type contains action authority, tools, Member Direction, Filing Decisions, Filing Rules or duplicate decisions.

## Options considered

### Luna-managed remote LiteLLM gateway — selected

This keeps upstream credentials server-side, provides one multi-provider harness, supports narrow client authentication and avoids shipping Python. It adds an operated service and requires strict request-body logging controls.

### Local LiteLLM sidecar — rejected for the first vertical

A sidecar would add Python packaging, lifecycle, patching and process-isolation work to every desktop installation. It would also move managed upstream credential handling onto the Trusted Device. A future Local-only Intelligence distribution may use a different local adapter without changing Document Handling.

### Direct provider adapters — retained as a replacement option

Direct Rust adapters reduce proxy infrastructure but duplicate provider transport and structured-output behaviour. They remain suitable for future Bring-your-own Intelligence or if LiteLLM fails evaluation. They must implement the same Luna-owned gateway contract.

## Provider selection and fallback restrictions

- Luna approves and names the Intelligence Provider and model before invocation.
- A Consent Grant is restricted to that provider, model, capability and disclosed scope. Reusable consent additionally binds the current media type and locally known context values shown to the member; changed scope fails closed and requires a new grant.
- One-time consent is persisted and consumed on its first transmission attempt.
- No LiteLLM router, alias, load balancer or fallback may select another provider or materially different model.
- Provider or gateway failure leaves Document Handling waiting with the Original and Local Inspection Evidence intact.
- A named alternative requires a separate applicable Consent Grant.
- Cloud completion and later candidate acceptance, correction or rejection are separate immutable History facts; Luna never rewrites the earlier event.

## Consequences and replacement

LiteLLM is an infrastructure detail, not an irreversible platform choice. Adding Anthropic, Gemini, Azure, Bedrock, a supported local endpoint, Portkey or a direct adapter changes the provider catalogue and gateway implementation, not Document Handling workflows. Before LiteLLM becomes long-lived infrastructure, Luna must evaluate it against Portkey and direct adapters for privacy controls, structured-output fidelity, failure semantics, operating burden, cost metadata and provider coverage.

The remote deployment must pin and evaluate a LiteLLM version, disable raw request/response and message-content logging, keep ordinary ingress logs body-free, verify those controls after upgrades and retain only privacy-safe usage metadata. LiteLLM's own logging switches are defence in depth rather than the sole privacy control.
