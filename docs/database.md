# Database

Luna uses PostgreSQL as the structured source of truth. Original files are stored separately and referenced by database records.

## Core Tables

- users: accounts that own or access Luna data.
- workspaces: personal, family, landlord, or business spaces.
- documents: original uploaded or emailed files.
- bills: extracted bill and invoice records.
- extraction_runs: AI extraction attempts and confidence metadata.
- audit_events: important user and system actions.

## Bill Statuses

- draft: extracted but not confirmed.
- unpaid: confirmed and awaiting payment.
- paid: marked as paid or matched to a payment.
- overdue: unpaid and past due date.
- archived: retained but hidden from active workflows.

## Storage Rule

Every bill or invoice should point back to exactly one original document where possible. The document is the single source of truth; extracted fields can be corrected, but the original remains unchanged.

## Schema Draft

See [schema.sql](../infrastructure/postgres/schema.sql).
