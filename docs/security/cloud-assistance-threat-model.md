# Cloud Assistance threat model

## Assets

- untouched Originals and locally extracted content;
- Household Context, Filing Rules, History and member identity;
- Luna gateway access credentials;
- upstream Intelligence Provider credentials;
- provider/model selection and Consent Grants.

## Trust boundaries

The Trusted Device owns protected Document Handling state and consent policy. The separately operated Luna-managed gateway is an external processor, even when an operator runs the prototype canary on loopback. LiteLLM and the selected Intelligence Provider are untrusted for authority and durable state.

## Threats and controls

| Threat | Control |
| --- | --- |
| Silent document upload | Local Inspection runs first; difficult Documents enter `NeedsCloudConsent`; the interface names the provider/model, disclosure and Keep local choice. |
| Consent widening | Grants bind provider, model, capability, disclosed fields, member, time and one-time/document or reusable future scope. Reusable scope also binds the media type and locally known context values shown before consent; any wider or changed request requires a new grant. Revocation is inspectable. |
| Cross-provider, model or billing fallback | Luna validates the exact route; LiteLLM retries/fallbacks are disabled; Luna retries only the identical route. BYOK runs in a separate process with no Luna-funded provider key, and free-Household virtual keys expose only BYOK model names. |
| Excessive disclosure | The Rust application layer builds the request from protected state and caps extracted text at 4,000 characters. The frontend cannot provide request content. |
| Provider-owned authority | Results contain Evidence and candidate Direction Interpretations only. Owning-domain validation rejects attempts to replace resolved Member Direction and validates monetary and ISO-date constraints before presentation; no tools are exposed. |
| Prompt or structured-output injection | Strict structured output, correlation/identity checks, allowed-field filtering, value limits and owning-domain validation. Provider text never reaches command dispatch. Completion and later candidate disposition are separate immutable History events. |
| Credential disclosure | Managed upstream credentials are server-side. Luna automatically provisions the narrow desktop gateway credential into the OS vault; members cannot paste it through the interface. Customer provider keys are tested and retained only in the OS vault, then forwarded separately from gateway authentication. Errors expose bounded Luna failure categories. |
| Content in infrastructure logs | Ordinary ingress logs must omit bodies and headers. LiteLLM message/raw logging is disabled, callbacks are absent, prompts are excluded from spend logs and upgrade verification is mandatory. |
| Replay of one-time consent | One-time Consent Grants are durably recorded and marked consumed before transmission. |
| Failure mistaken for completion | Document Handling moves to a recoverable waiting state; the Original and Local Inspection Evidence remain unchanged. |

## Credential ownership

The operating-system-vault acceptance criterion applies to the desktop's automatically provisioned Luna gateway access credential and Bring-your-own Intelligence credentials. For Luna-managed Intelligence, the upstream OpenAI credential is owned by the separately operated gateway environment and never delivered to the desktop application or its storage.

The gateway credential must be narrow, revocable and attributable for abuse controls. It must not be customer-entered or written to environment files in the repository, frontend storage, SQLite, Cabinet content, History or diagnostics.

A Bring-your-own provider credential may be entered only through Luna, must pass a synthetic connection test before save, and is retained only in the OS vault. It can be replaced or removed only through Options. It transits a separate BYOK gateway using a BYOK-only virtual key and route and can never fall back to a Luna-funded deployment. Header, application, error and spend logs plus a PostgreSQL dump must prove that the credential and test content are neither retained nor emitted.

## Deployment verification

Before a gateway release:

1. pin the LiteLLM image/version;
2. verify only the allowlisted provider/model is addressable by a desktop credential;
3. inspect ingress, application, error and spend logs with a canary document string;
4. prove the canary and bearer token are absent;
5. prove provider/model fallback is disabled;
6. revoke the test virtual key and prove subsequent authentication fails;
7. record safe usage metadata and delete the test request.

LiteLLM configuration flags are not the only control; network ingress and log sinks must independently prevent body capture.

For prototype acceptance, this verification may use the pinned Compose deployment bound to loopback, a fixed synthetic request and disposable credentials. That exception does not authorise a desktop sidecar or real Household information. Before external testing, the gateway must run remotely behind authenticated TLS ingress with managed secrets, attributable client credentials and abuse controls; issue #53 owns that pre-production evidence.
