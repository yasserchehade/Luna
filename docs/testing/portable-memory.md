# Portable Memory verification

Issue #15 is verified through the public `PortableMemoryStore` local-core interface and the owning `ConversationStore` and `CloudIntelligenceStore` seams in `desktop/src-tauri/tests/portable_memory.rs`.

The integration scenarios use real temporary SQLite databases, real Cabinet folders and two independent in-memory credential-vault adapters representing separate Trusted Devices. They do not inspect SQLite as a test shortcut.

Run the focused suite:

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml --test portable_memory
```

The suite demonstrates:

- Household-key encryption, Trusted Device signatures and an append-only reserved Cabinet area;
- automatic capture from real Filing Rule, duplicate, Member Direction, Consent and Cloud Assistance behavior;
- complete Filing Rule, relationship, authority, Consent Grant and History reconstruction into the recovered device's owning local stores;
- actual use of a rebuilt Filing Rule to file a later matching Document automatically;
- History presentation through the desktop History surface;
- local-first recording and exact delivery after Cabinet recovery;
- an explicit unavailable-Cabinet result distinct from an empty portable-memory area;
- idempotent duplicate delivery and causal out-of-order import;
- tamper, replay, device-substitution and post-revocation rejection;
- historical-key recovery after Household-key rotation;
- withheld mutable projections during direct or competing-resolution conflicts;
- closed Consent field types, typed-schema rejection of credential-shaped fact content, and a real provider credential remaining exclusively in the device vault; and
- absence of a live database file from the Cabinet.

The focused suite contains 15 scenarios. The full repository gate additionally exercises offline Document Handling, Cabinet recovery and cross-platform application behavior through their owning public seams. Account-contract verification covers the authorization public keys, activation epochs and signed-record revocation cutoffs supplied to synchronization.
