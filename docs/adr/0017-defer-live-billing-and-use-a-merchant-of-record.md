---
status: accepted
---

# Defer live billing and use a merchant of record

> **Deferred for the MVP by [ADR 0019](./0019-openai-mvp-household-administration-agent.md).** This record remains historical. Extensive billing infrastructure is not part of proving email-to-household-work-to-action.

The first prototype grants a small, usage-capped cohort complimentary Managed Intelligence Entitlements and does not accept live payments. Luna will exercise Paddle sandbox as the initial merchant-of-record billing adapter because its transaction-funded model includes hosted checkout, subscriptions, customer self-service, tax handling and billing support without a monthly platform charge. Live billing remains disabled until issue #53 can automatically provision and revoke working managed access; the Household Plan and entitlement model remains independent of Paddle so another billing adapter can replace it without changing Luna's domain.
