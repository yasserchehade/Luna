---
status: superseded
superseded_by: 0019
---

# Keep Luna local-first and intelligence-provider neutral

> **Superseded for the MVP by [ADR 0019](./0019-openai-mvp-household-administration-agent.md).** This record remains as historical context. Luna still owns household memory, authority, tools and audit, but the MVP now uses OpenAI as the single reasoning and document-reading engine and defers local-only, BYOK and multi-provider product choices.

Luna owns household context, authority, tools, learned rules and audit history; deterministic inspection happens locally and external reasoning requires a provider-specific consent grant. We rejected binding Luna to a consumer AI account or one model vendor so households can choose Luna-managed, bring-your-own or local-only intelligence without losing Luna's memory or changing its authority.
