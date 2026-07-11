---
status: accepted
---

# Synchronise portable events instead of a live database

Each trusted device will keep its own local working database, while signed, encrypted, append-only Luna records travel in a reserved area beside the cabinet. We rejected synchronising a live database because consumer cloud drives can copy partial writes and create conflicting versions; the event approach costs more merge engineering but preserves user-owned portability and makes conflicts explicit.
