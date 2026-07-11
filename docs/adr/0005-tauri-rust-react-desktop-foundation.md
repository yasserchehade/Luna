---
status: accepted
---

# Build Luna as a Tauri desktop application

Luna will use a Tauri desktop shell, React interface and Rust local core for its Windows and macOS foundation. We chose an installed local-first application over a browser-first service, and Tauri over a heavier Electron runtime, because filesystem custody, credential-vault access, background work, offline behaviour and a small trusted computing footprint are central to the product despite the additional Rust engineering cost.
