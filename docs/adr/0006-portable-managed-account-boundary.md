---
status: accepted
---

# Use Supabase behind a portable Luna account boundary

Luna will use managed Supabase Auth and PostgreSQL for beta account verification, sessions and Household membership, but Luna will own stable account identifiers and map Supabase identities through a provider-neutral Account Service. Schema and access policies remain versioned migrations, and Supabase will hold only minimal identity and coordination data—never cabinet documents, readable household memory, provider credentials, device private keys or decryption keys—so the service can later be self-hosted or replaced without redesigning Household ownership.
