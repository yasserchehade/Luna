# Portable Memory verification

Issue #15 is verified through the public `PortableMemoryStore` local-core interface in `desktop/src-tauri/tests/portable_memory.rs`.

The integration scenarios use real temporary SQLite databases, real Cabinet folders and two independent in-memory credential-vault adapters representing separate Trusted Devices. They do not inspect SQLite as a test shortcut.

Run the focused suite:

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml --test portable_memory
```

The suite demonstrates:

- Household-key encryption, Trusted Device signatures and an append-only reserved Cabinet area;
- complete Filing Rule, relationship, authority, Consent Grant and History reconstruction on a recovered Trusted Device;
- local-first recording and exact delivery after Cabinet recovery;
- an explicit unavailable-Cabinet result distinct from an empty portable-memory area;
- idempotent duplicate delivery and causal out-of-order import;
- tamper, replay, device-substitution and post-revocation rejection;
- historical-key recovery after Household-key rotation;
- withheld mutable projections during direct or competing-resolution conflicts;
- typed-schema rejection of credentials, tokens, private keys and secret-shaped fact fields; and
- absence of a live database file from the Cabinet.

The full repository gate additionally exercises consent denial, provider failure, offline Document Handling, Cabinet recovery and cross-platform application behavior through their owning public seams.
