---
status: accepted
---

# Separate managed and bring-your-own intelligence billing

Intelligence access is entitled at the Household level. An eligible paid Household plan includes Luna-managed Intelligence whose approved provider usage is billed to Luna; a free Household can configure Bring-your-own Intelligence through Luna's interface and have usage billed by the selected provider to the connection owner, or remain Local-only. Paid Households may also choose Bring-your-own or Local-only Intelligence so payment never removes provider choice.

Customer setup remains entirely inside Luna. A managed Household never receives or pastes Luna's upstream provider key or gateway credential; device access to the managed gateway is provisioned automatically. A Bring-your-own provider credential is entered, tested, replaced and removed through Luna's interface and protected according to its declared scope.

Issue #55 selected a separate BYOK-only LiteLLM process with no Luna-funded provider credential. Luna authenticates the Trusted Device with `x-litellm-api-key` and forwards the customer credential separately as `x-api-key` only for the selected provider request. Customer keys remain in the OS vault between requests. BYOK virtual keys expose only BYOK model names, retries and fallbacks are disabled, and a missing or rejected customer credential can never select the managed deployment.
