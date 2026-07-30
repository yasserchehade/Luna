---
status: accepted
---

# Own the Intelligence Gateway and isolate LiteLLM

Luna will own a provider-neutral `IntelligenceGateway` contract and use an isolated, separately operated LiteLLM proxy as the provisional first Luna-managed implementation. Document Handling and Conversation depend only on Luna-owned request, result, failure, Consent Grant and Direction Interpretation types. LiteLLM translation, HTTP details and error mapping remain inside one private Rust adapter.

## Context

Cloud Assistance must support more than one Intelligence Provider without allowing an external harness to own Household authority, privacy policy, provider selection, durable state or consequential actions. The first implementation also must keep Luna's upstream provider credentials out of the desktop, avoid making Python a desktop-wide dependency and remain testable without paid provider calls.

## Decision

The first evaluated Luna-managed route is OpenAI `gpt-4.1-mini` through an isolated LiteLLM gateway. Luna selects that exact Intelligence Provider and model, validates a provider-and-model-specific Consent Grant, constructs a bounded structured request, and only then calls the gateway. The adapter sends `openai/gpt-4.1-mini` explicitly and disables LiteLLM retries and fallbacks. Luna may perform a bounded safe retry using the unchanged request, provider and model.

The desktop authenticates with a narrow, revocable Luna gateway credential held in the operating-system credential vault. The separately operated gateway holds the upstream OpenAI credential. Neither credential enters SQLite, Cabinet files, History, request content or diagnostics. Production gateway keys are attributable to a Trusted Device or Household for abuse controls and may be independently revoked.

That gateway credential is managed infrastructure rather than customer configuration. Eligible paid Households receive managed gateway access through automatic device provisioning; no member pastes a Luna gateway or upstream provider credential. ADR 0016 separately defines the Household-level paid managed, Bring-your-own and Local-only product modes.

LiteLLM output is untrusted. Luna verifies correlation, provider, model, allowed fields, value constraints and the structured response before converting it to Evidence or a candidate Direction Interpretation. The owning Document Handling domain validates that candidate again, including that the field still awaits Member Direction and that monetary and ISO-date values satisfy domain constraints. No result type contains action authority, tools, Member Direction, Filing Decisions, Filing Rules or duplicate decisions.

## Prototype evaluation boundary

The evaluated-real-provider canary may run against the pinned LiteLLM Compose deployment on an operator's loopback interface. This is an ephemeral release-environment test using synthetic content and disposable credentials; it is not a desktop sidecar, a member-facing deployment or permission to use real Household information. LiteLLM remains outside the desktop process and lifecycle, and the upstream credential exists only in the operator-controlled gateway environment.

Before Luna is released to external testers, the same gateway contract must be operated remotely behind authenticated TLS ingress, managed secrets, attributable client credentials, body-free infrastructure logs and abuse controls. That pre-production deployment gate is tracked separately from the prototype's provider-contract evaluation.

## Options considered

### Luna-managed isolated LiteLLM gateway — selected

This keeps upstream credentials server-side, provides one multi-provider harness, supports narrow client authentication and avoids shipping Python. It adds an operated service and requires strict request-body logging controls.

### Desktop LiteLLM sidecar — rejected for the first vertical

A sidecar would add Python packaging, lifecycle, patching and process-isolation work to every desktop installation. It would also move managed upstream credential handling onto the Trusted Device. A future Local-only Intelligence distribution may use a different local adapter without changing Document Handling.

### Direct provider adapters — retained as a replacement option

Direct Rust adapters reduce proxy infrastructure but duplicate provider transport and structured-output behaviour. They remain suitable for future Bring-your-own Intelligence or if LiteLLM fails evaluation. They must implement the same Luna-owned gateway contract.

## Provider selection and fallback restrictions

- Luna approves and names the Intelligence Provider and model before invocation.
- Options persists one exact Default Intelligence Provider and model. Conversation replies and Document evaluations require separate capability-specific Default Intelligence Permissions. Changing or disabling the default revokes both permissions instead of transferring them.
- A Consent Grant is restricted to that provider, model, capability and disclosed scope. Reusable consent additionally binds the current media type and locally known context values shown to the member; changed scope fails closed and requires a new grant.
- An ordinary Conversation request contains only the newly submitted message. It never contains earlier messages, Documents, Household state, tools, action authority or Luna-owned request and route identities. The provider returns only a bounded reply; the synchronous adapter attaches the request, correlation, provider and model identities locally before common validation. A validated reply can only become a Luna-authored Conversation message.
- A member-initiated Document evaluation uses the approved default route and enumerated Document field set. The Review Card does not offer a second provider/model or per-Document consent selector, and Conversation cannot create one-time Document consent. Historical one-time grants remain readable for audit and compatibility but cannot authorise the default path.
- No LiteLLM router, alias, load balancer or fallback may select another provider or materially different model.
- Provider or gateway failure leaves Document Handling waiting with the Original and Local Inspection Evidence intact.
- A named alternative requires a separate applicable Consent Grant.
- Cloud completion and later candidate acceptance, correction or rejection are separate immutable History facts; Luna never rewrites the earlier event.

## Consequences and replacement

LiteLLM is an infrastructure detail, not an irreversible platform choice. Adding Anthropic, Gemini, Azure, Bedrock, a supported local endpoint, Portkey or a direct adapter changes the provider catalogue and gateway implementation, not Document Handling workflows. Before LiteLLM becomes long-lived infrastructure, Luna must evaluate it against Portkey and direct adapters for privacy controls, structured-output fidelity, failure semantics, operating burden, cost metadata and provider coverage.

Every deployment must pin and evaluate a LiteLLM version, disable raw request/response and message-content logging and retain only privacy-safe usage metadata. A pre-production or production remote deployment must additionally keep ordinary ingress logs body-free and verify those controls after upgrades. LiteLLM's own logging switches are defence in depth rather than the sole privacy control.
