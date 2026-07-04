# Storage

Luna should be local-first and user-owned by default.

The household archive should not automatically become Luna company cloud storage. Original documents are sensitive family records, so Luna should let the household choose where the cabinet lives: a local drive, external drive, NAS, iCloud Drive, Google Drive, OneDrive, Dropbox, or another user-owned storage location.

Luna's job is to organize, understand, and coordinate the archive. The user owns the storage.

## Storage Principles

- Original documents stay in user-owned storage by default.
- Luna-managed cloud document storage is optional, not required.
- The graph is the source of truth; folders are a human-readable projection of the graph.
- Luna should suggest and prepare filing decisions before moving files when confidence is low.
- Luna should not silently delete, overwrite, or irreversibly move user files.
- A document may relate to many graph nodes, even if the physical file has one canonical location.

## Household Cabinet

Luna should create a logical household cabinet that the user can open outside Luna at any time.

Example:

```text
Luna Household Cabinet/
  Inbox/
    Needs Review/
  Family Trust/
    Properties/
      Property 1/
        Bills/
        Insurance/
        Maintenance/
        Suppliers/
        Documents/
    Business/
      Business 1/
        Invoices/
        Tax/
        MYOB/
        Suppliers/
    Suppliers/
    Documents/
    Tasks/
  Family Members/
  Vehicles/
  Warranties/
  Tax/
  Archived/
```

The exact structure should come from the user's household graph. If a household connects `FamilyMember -> FamilyTrust -> Property -> Supplier -> Document`, Luna can project that relationship into a cabinet path. Another household may use a different structure.

## Canonical Files and Graph Links

For MVP, Luna should keep one canonical file path per document. Extra relationships live in the database graph.

Example: an insurance invoice may relate to a trust, property, supplier, tax year, bill, and reminder. The file might physically live under:

```text
Family Trust/Properties/Property 1/Insurance/2026-05-29_IMC-Insurance-Brokers_Invoice-291727_2405.08.pdf
```

Luna still records all other relationships in the graph, so the app can show the same document from the property, supplier, tax, bill, or task context without duplicating the file.

## MVP Storage Behavior

Phase 1 should stay safe and simple:

1. Store uploaded files in a local Luna storage area.
2. Record the original filename, content hash, storage provider, and current storage path.
3. Generate a suggested cabinet path from extracted fields and graph relationships.
4. Mark uncertain documents as `needs_review`.
5. Do not physically move or rename files until the document is confirmed.
6. After confirmation, copy or move the file into the cabinet path through an explicit user action.

The first implementation generates suggested paths without moving files, then exposes a separate filing action. Filing defaults to `copy`, stores the file under `CABINET_STORAGE_PATH`, updates the document's canonical storage path, marks the document as `filed`, and records an audit event. `move` is available only when explicitly requested by the API caller.

## Storage Providers

The storage layer should eventually support:

- `local_folder`
- `icloud_drive_folder`
- `google_drive_folder`
- `onedrive_folder`
- `dropbox_folder`
- `nas_folder`
- `luna_encrypted_cloud` as an optional paid backup or sync feature

For MVP, implement `local_folder` first, but keep the data model provider-aware.

## Server Storage Boundary

Luna company servers should store only what is needed for the chosen product mode.

Minimum server data:

- Account and subscription records.
- Device or workspace registration.
- Optional encrypted sync state.
- Optional integration connection metadata.
- AI usage and billing counters.

User-owned by default:

- Original PDFs and document files.
- Full extracted document text.
- Local search index.
- Sensitive family, health, school, tax, insurance, government, and financial records.

If cloud processing is enabled, Luna should send the smallest useful context and make that behavior clear to the user.
