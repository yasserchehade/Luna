# Database

Luna uses PostgreSQL as the structured source of truth. Original files are stored separately in user-owned cabinet storage and referenced by database records.

The database should evolve from the current bills MVP into a household knowledge graph: typed entities, source documents, relationships, tasks, reminders, extraction runs, and audit events.

## Current Schema

The current implementation includes:

- `workspaces`: current ownership boundary for personal, family, landlord, or business spaces.
- `users`: accounts that own or access Luna data.
- `workspace_memberships`: user roles inside a workspace.
- `documents`: original uploaded or emailed files, including storage provider, current storage path, cabinet status, suggested cabinet path, and confirmed cabinet path.
- `document_texts`: extracted text and page metadata for stored documents.
- `bills`: extracted bill and invoice records, including extraction confidence, review status, and review reasons.
- `household_entities`: typed household graph nodes such as suppliers, properties, vehicles, subscriptions, and assets.
- `entity_relationships`: graph links between documents, entities, obligations, and source evidence.
- `supplier_profiles`: known supplier extraction profiles, aliases, categories, and review status.
- `supplier_template_versions`: observed supplier template fingerprints and expected anchors.
- `document_template_matches`: per-document match records showing whether a supplier template looked known, unknown, or changed.
- `tasks`: review or action items created by users, integrations, or AI workflows.
- `reminders`: scheduled prompts tied to bills or other household entities.
- `extraction_runs`: AI extraction attempts and confidence metadata.
- `audit_events`: append-only history for important user and system actions such as document uploads, cabinet confirmations, graph edits, bill lifecycle changes, task/reminder changes, and assistant answers.

See [schema.sql](../infrastructure/postgres/schema.sql).

## Target Household Entities

The data model should grow toward these first-class entities:

- Household
- User
- FamilyMember
- Business
- Property
- Vehicle
- Supplier
- Document
- Bill
- Invoice
- Payment
- BankAccount
- InsurancePolicy
- UtilityAccount
- Subscription
- Task
- Reminder
- CalendarEvent
- SchoolObligation
- HealthEvent
- GovernmentDocument
- TaxObligation
- Warranty
- Asset

## Relationship Model

Most useful household knowledge comes from relationships, not isolated records. Luna should be able to represent facts such as:

- A document is from a supplier.
- A bill relates to a property and a utility account.
- A payment settles an invoice.
- An insurance policy covers a vehicle or property.
- A warranty covers an asset.
- A school obligation belongs to a family member and calendar event.
- A tax obligation relates to a business, property, document, or payment.

A future generalized relationship table can support this:

```text
entity_relationships
  id
  household_id
  source_entity_type
  source_entity_id
  relationship_type
  target_entity_type
  target_entity_id
  provenance_document_id
  confidence
  created_by
  created_at
```

Use explicit foreign keys for high-volume critical workflows when they are stable, and use generalized relationships for graph traversal, discovery, and cross-domain links.

## Bill Statuses

- `draft`: extracted but not confirmed.
- `unpaid`: confirmed and awaiting payment.
- `paid`: marked as paid or matched to a payment.
- `overdue`: unpaid and past due date.
- `archived`: retained but hidden from active workflows.

## Storage Rule

Every bill or invoice should point back to an original document where possible. The document is the source of truth; extracted fields can be corrected, but the original remains unchanged.

Document records include provider-aware cabinet references:

```text
documents
  storage_provider
  storage_path
  original_filename
  cabinet_status
  suggested_cabinet_path
  confirmed_cabinet_path
  sha256
```

`storage_provider` can represent local folders first, then user-owned cloud folders such as iCloud Drive, Google Drive, OneDrive, Dropbox, or NAS. Luna-managed encrypted cloud storage should be an optional provider, not the default.

The graph should drive cabinet path suggestions. A document linked to a family trust, property, supplier, and bill can receive a suggested path under the relevant trust or property while keeping all relationships in the database.

## Planning Notes

- Keep household or workspace ownership on every user-visible record.
- Store AI confidence and provenance for extracted fields and relationships.
- Prefer append-only audit events for meaningful changes. Audit metadata should explain the action without copying full sensitive source documents into the log.
- Make tasks and reminders generic enough to attach to any household entity.
- Avoid hard-coding the MVP around bills in a way that blocks properties, vehicles, insurance, school, health, government, or tax workflows later.
