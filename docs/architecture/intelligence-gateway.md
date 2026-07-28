# Intelligence Gateway architecture

ADR 0015 implements ADR 0003's provider-neutral Cloud Assistance boundary.

```mermaid
flowchart TD
    Handling["Document Handling"] --> Inspect["Local Inspection"]
    Inspect --> Need["Local Evidence is insufficient"]
    Need --> Capability["Luna selects Direction Interpretation"]
    Capability --> Selection["Luna selects OpenAI + gpt-4.1-mini"]
    Selection --> Consent["Validate or request provider/model-specific Consent Grant"]
    Consent --> Request["Build bounded Intelligence Request"]
    Request --> Gateway["Luna-owned Intelligence Gateway"]
    Gateway --> Adapter["Private LiteLLM adapter"]
    Adapter --> Deployment["Isolated LiteLLM deployment"]
    Deployment --> Provider["Exact OpenAI model"]
    Provider --> Untrusted["Structured untrusted result"]
    Untrusted --> Validation["Luna schema, identity and correlation validation"]
    Validation --> Domain["Document Handling validation"]
    Domain --> Evidence["Evidence or candidate Direction Interpretation"]
```

## Public boundary

`IntelligenceGateway` receives a Luna-owned `IntelligenceRequest` and returns a Luna-owned `UntrustedIntelligenceResult` or `IntelligenceFailure`. The request carries a safe request identifier, Document Arrival correlation, capability, exact provider/model, selected Local Inspection Evidence, bounded content excerpts, expected response schema, Consent Grant reference and execution constraints.

`DocumentIntelligenceService` is the application seam joining protected Document Handling state to that boundary. The frontend supplies only a Document Arrival identifier, exact selection, consent choice, granting Luna Account and optional existing Consent Grant. It cannot construct an arbitrary provider payload.

The production catalogue contains two deliberately separate OpenAI routes:

| Intelligence Provider | Model | Mode | Harness |
| --- | --- | --- | --- |
| OpenAI | `gpt-4.1-mini` | Luna-managed Intelligence | isolated LiteLLM; remote before external testing |
| OpenAI | `gpt-4.1-mini` | Bring-your-own Intelligence | separate BYOK-only LiteLLM process; customer provider key required |

Luna-managed Intelligence is an entitlement of an eligible paid Household plan and uses provider credentials billed to Luna. A free Household may instead configure a supported Bring-your-own provider connection entirely through Luna's interface, with provider usage billed to the connection owner, or remain Local-only. Paid Households retain those choices. Managed gateway credentials are automatically provisioned to Trusted Devices and are never customer-entered settings.

The managed adapter authenticates with a bearer gateway credential. The BYOK adapter sends the automatically provisioned gateway credential as `x-litellm-api-key` and the transient customer provider credential separately as `x-api-key`. The provider credential is loaded from the OS vault and never enters the request body. Options keeps provider-key entry disabled until that Trusted Device has BYOK gateway access, so missing infrastructure access is never misreported as a bad customer key. The BYOK LiteLLM process has no `OPENAI_API_KEY`; its configured model is `byok/openai/gpt-4.1-mini`, and its virtual keys cannot address the managed route.

## Household Plan and billing

A Managed Intelligence Entitlement belongs to the Household and remains distinct from Trusted Device provisioning. During the first prototype, a small beta cohort receives complimentary, usage-capped entitlements through the same server-side interface that will later consume paid subscription state. No account identity is hard-coded and a checkout redirect can never grant access.

Paddle sandbox is the first Billing Subscription adapter evaluated under ADR 0017. Luna creates checkout and customer-portal sessions only for an authenticated Household Organiser, accepts entitlement changes only from verified and idempotently processed server-side billing events, and retains only external billing identifiers plus access-critical status. Card data and billing secrets never enter the desktop, Cabinet or protected Household state.

Live charging remains disabled until issue #53's remote gateway can provision and revoke narrow managed credentials for entitled Trusted Devices and enforce abuse controls. Bring-your-own and Local-only Intelligence remain available regardless of Billing Subscription state.

`DeterministicIntelligenceGateway` implements the same contract for tests and never enters the production registry.

For prototype acceptance, an operator may run the pinned gateway deployment ephemerally on loopback and send only the fixed synthetic canary. The desktop and canary accept cleartext HTTP only for loopback hosts and reject every cleartext remote endpoint before transmitting credentials or content. This validates the real provider contract without adding LiteLLM or Python to the desktop. Before external testing, the same boundary moves behind authenticated remote HTTPS ingress with managed secrets and attributable gateway credentials.

## Minimisation

For Direction Interpretation, Luna may transmit:

- media type;
- local values for unresolved document fields;
- the names of fields Luna asks the provider to interpret;
- at most 4,000 characters of locally extracted text.

Luna does not transmit the Household identifier, Cabinet or source paths, checksum, Filing Rules, duplicate state, History, credentials, the Original file, or complete Household state. Relevance to a Household, property or Service Provider remains Member Direction and is not requested from the provider.

Before Luna offers reusable consent, the Conversation states its concrete future scope: the exact provider/model and capability, the current media type, the same locally known context values and no wider set of disclosed fields. The protected Consent Grant stores that local-scope Evidence and validates it against every attempted reuse. A changed media type, local context value, provider, model, capability or wider disclosed field set requires a new Consent Grant.

## Validation and authority

The gateway result must echo the request, Document Arrival, provider and model identities. Luna rejects unknown fields, empty or oversized values, malformed structured results and mismatched correlation. Document Handling then rejects candidate fields that already have Member Direction, malformed monetary values and dates that are not valid ISO calendar dates, in addition to its existing bounds and control-character constraints.

The provider result cannot create Member Direction or mutate a Document Arrival. A validated candidate is held in the review interface until a Household Member accepts or corrects it through the existing Member Direction command. History records provider completion and later candidate acceptance, correction or rejection as separate immutable events; an earlier event is never rewritten.

## Failure and retry

Luna maps infrastructure failures to its own categories. Only `ProviderUnavailable`, `GatewayUnavailable` and `TimedOut` receive one bounded retry, using the byte-equivalent Luna request and the same provider/model. LiteLLM receives `num_retries: 0` and an empty fallback list.

Exhausted, invalid, rejected, gateway-authentication, provider-credential or rate-limit failures leave Document Handling in `WaitingForCloudAssistance`. For BYOK 401/403 responses, the adapter reads only a bounded structured error type: known LiteLLM virtual-key failures remain gateway-authentication failures, while upstream authentication failures are attributed to the selected provider credential. A missing BYOK provider key is rejected before transmission and before one-time consent is consumed. Keep local returns it to `NeedsMemberDirection` without a gateway call. Existing Filing Rules and duplicate/version handling never call this gateway and remain offline-capable.
