---
status: accepted
---

# Synchronise portable events instead of a live database

Each trusted device will keep its own local working database, while signed, encrypted, append-only Luna records travel in a reserved area beside the cabinet. We rejected synchronising a live database because consumer cloud drives can copy partial writes and create conflicting versions; the event approach costs more merge engineering but preserves user-owned portability and makes conflicts explicit.

Each record is encrypted with the Household key and signed with the originating Trusted Device's separate authorisation key. Import verifies the device signature and event chain before decrypting or changing local state. Duplicate delivery is idempotent; modified or replay-invalid records are rejected. Concurrent mutable facts retain both valid records and create a resolvable Portable Memory Conflict rather than applying last-write-wins.

The portable schema is deliberately typed around durable domain facts. It has no field for credentials, private keys, derived Conversation Prompts, transient Conversation Orchestration, hidden reasoning or raw Intelligence Provider output.
