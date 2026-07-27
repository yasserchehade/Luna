# Cloud Assistance threat model

## Assets

- untouched Originals and locally extracted content;
- Household Context, Filing Rules, History and member identity;
- Luna gateway access credentials;
- upstream Intelligence Provider credentials;
- provider/model selection and Consent Grants.

## Trust boundaries

The Trusted Device owns protected Document Handling state and consent policy. The Luna-managed remote gateway is an external processor. LiteLLM and the selected Intelligence Provider are untrusted for authority and durable state.

## Threats and controls

| Threat | Control |
| --- | --- |
| Silent document upload | Local Inspection runs first; difficult Documents enter `NeedsCloudConsent`; the interface names the provider/model, disclosure and Keep local choice. |
| Consent widening | Grants bind provider, model, capability, disclosed fields, member, time and one-time/document or reusable future scope. Revocation is inspectable. |
| Cross-provider or model fallback | Luna validates the exact route; LiteLLM retries/fallbacks are disabled; Luna retries only the identical route. |
| Excessive disclosure | The Rust application layer builds the request from protected state and caps extracted text at 4,000 characters. The frontend cannot provide request content. |
| Provider-owned authority | Results contain Evidence and candidate Direction Interpretations only. Owning-domain validation precedes Member Direction; no tools are exposed. |
| Prompt or structured-output injection | Strict structured output, correlation/identity checks, allowed-field filtering, value limits and owning-domain validation. Provider text never reaches command dispatch. |
| Credential disclosure | Upstream credentials are server-side. The narrow desktop gateway credential is stored only in the OS vault. Errors expose bounded Luna failure categories. |
| Content in infrastructure logs | Ordinary ingress logs must omit bodies and headers. LiteLLM message/raw logging is disabled, callbacks are absent, prompts are excluded from spend logs and upgrade verification is mandatory. |
| Replay of one-time consent | One-time Consent Grants are durably recorded and marked consumed before transmission. |
| Failure mistaken for completion | Document Handling moves to a recoverable waiting state; the Original and Local Inspection Evidence remain unchanged. |

## Credential ownership

The operating-system-vault acceptance criterion applies to the desktop's Luna gateway access credential and any future Bring-your-own Intelligence credential. For Luna-managed Intelligence, the upstream OpenAI credential is owned by the remote service and never delivered to the desktop.

The gateway credential must be narrow, revocable and attributable for abuse controls. It must not be written to environment files in the repository, frontend storage, SQLite, Cabinet content, History or diagnostics.

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
