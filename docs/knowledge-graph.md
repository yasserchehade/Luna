# Knowledge Graph

Luna's core product asset is a structured knowledge graph of household life, governed by an authority model.

The graph connects documents, people, properties, vehicles, accounts, bills, payments, obligations, events, reminders, tasks, suppliers, policies, warranties, assets, roles, and authority. This is what lets Luna answer questions, create useful reminders, prepare work, request approval, and coordinate across existing systems.

The graph must be flexible enough for each household to model its own reality. One household might connect `FamilyMember -> FamilyTrust -> Property -> Documents -> Suppliers -> MaintenanceItems -> DueDates`. Another might connect a business, vehicle, insurance policy, bank account, and tax obligation differently. Luna should provide useful defaults without forcing every family into the same structure.

Most family structure should be long-lived and change infrequently after setup. Operational records such as bills, tasks, reminders, due dates, maintenance items, and review work attach to the stable graph while they are relevant. When they are no longer active, Luna should archive them rather than delete them, preserving history without cluttering the current dashboard.

## Graph-Driven Cabinet

The household cabinet should be generated from the user's graph. Luna should not force every family into one fixed folder taxonomy.

Example user structure:

```text
Family Member 1
  -> Family Trust
      -> Property 1
      -> Business 1
      -> Suppliers
      -> Documents
      -> Tasks
```

Possible cabinet projection:

```text
Luna Household Cabinet/
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
```

The graph is the source of truth. The folder structure is only a human-readable projection. A document can be related to a trust, property, supplier, tax year, bill, reminder, and task at the same time, while still having one canonical file path in the cabinet.

## Why a Knowledge Graph

Household administration is relational. A single document may involve a supplier, property, due date, payment, warranty, account, family member, and tax category. A simple list of bills cannot represent that context cleanly.

The knowledge graph lets Luna understand:

- What an item is.
- Who or what it relates to.
- Where the source information came from.
- What deadline or action it creates.
- What other records confirm, conflict with, or complete it.
- Whether Luna has authority to observe, prepare, write, or execute the next step.

## Initial Entity Types

These are starter types, not a closed list. Users should be able to create practical household node types such as `family_trust`, `maintenance_item`, or future domain-specific types as Luna grows.

- Household
- User
- FamilyMember
- HouseholdRole
- AuthorityPolicy
- ApprovalRequest
- WorkOrder
- FamilyTrust
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
- MaintenanceItem

## Example Relationships

- Household has FamilyMembers.
- FamilyMember has a HouseholdRole.
- AuthorityPolicy grants or limits Luna's work for a connection, work type, member, or approver.
- ApprovalRequest authorises, rejects, or escalates a WorkOrder.
- FamilyMember controls, benefits from, or administers a FamilyTrust.
- FamilyTrust owns or manages a Property.
- Household owns or manages Properties, Vehicles, Assets, and Businesses.
- Supplier issues a Bill or Invoice.
- Document is evidence for a Bill, Invoice, Policy, Warranty, or GovernmentDocument.
- Bill relates to a Property, Vehicle, Subscription, UtilityAccount, Business, or FamilyMember.
- MaintenanceItem relates to a Property, Supplier, Document, Task, Reminder, or Warranty.
- Payment settles a Bill or Invoice.
- InsurancePolicy covers a Property, Vehicle, Asset, or FamilyMember.
- Warranty covers an Asset or Vehicle.
- Reminder is triggered by a Bill, Policy, Warranty, CalendarEvent, TaxObligation, or SchoolObligation.
- WorkOrder represents prepared work such as review, pay, upload, renew, call, file, reconcile, compile, draft, or confirm.
- Task is created to review, approve, reject, correct, or complete a WorkOrder or entity.

## Provenance

Every extracted entity or relationship should keep provenance where possible:

- Source document or integration.
- Extracted field evidence.
- AI provider and model.
- Confidence score.
- User who confirmed or corrected the data.
- User who approved, rejected, delegated, or escalated a proposed action.
- Authority policy that allowed or blocked the action.
- Timestamp of creation or update.

Provenance matters because Luna will handle sensitive household information. Users need to trust not just the answer, but why Luna believes the answer.

## MVP Graph Behavior

Phase 1 should keep implementation simple while establishing graph habits:

- Store every uploaded bill or invoice as a Document.
- Extract a Bill or Invoice from the Document.
- Link the obligation to a Supplier.
- Link it to a Property, Business, UtilityAccount, Subscription, or FamilyMember when available.
- Generate a suggested cabinet path from graph relationships and extracted metadata.
- Create a Reminder from the due date.
- Create a Task or ApprovalRequest when human review or authority is needed.
- Preserve extraction confidence and source evidence.
- Preserve whether Luna's work is observed, prepared, proposed, approved, dismissed, or escalated.
- Treat family structure as durable context.
- Treat bills, tasks, reminders, due dates, and maintenance work as operational records that can be completed, dismissed, paid, or archived.

## Supplier Profiles

Supplier profiles are Luna's memory of how a recurring vendor usually formats documents. A profile can store supplier aliases, expected anchors, category hints, observed template fingerprints, and template status.

For a known supplier:

- If expected anchors are present, Luna can use the supplier profile confidently.
- If expected anchors are missing, Luna should mark the template as changed or needing review.
- If the supplier is unknown, Luna should use generic extraction and create or suggest a new profile after user review.

This keeps repeated invoices predictable while still allowing vendors to change templates over time.

## Visual Relationship Builder

The future GUI should behave like a relationship diagram:

- Entities are nodes.
- Relationships are edges.
- Users can create household-specific nodes such as family trusts, properties, suppliers, documents, maintenance items, tasks, and reminders.
- Users can drag or select two nodes and label the relationship, for example `owns`, `managed_by`, `supplied_by`, `requires_maintenance`, `covered_by`, or `due_for`.
- Luna can suggest relationships from documents, but users should be able to accept, edit, or remove them.
- The graph should remain explainable: every AI-created edge should retain provenance and confidence.

## Assistant Behavior

The assistant should answer from the graph and source documents. When possible, it should show the records behind an answer and identify uncertainty.

Good answer pattern:

- Direct answer.
- Related entity or document.
- Source and confidence.
- Suggested next action when useful.

Bad answer pattern:

- Ungrounded speculation.
- Hiding uncertainty.
- Acting without approval.
- Treating extracted data as final when it has not been reviewed.
