# Luna Competencies

A Luna competency is a household employee responsibility that Luna has learned to perform safely.

Tracer bullets remain the implementation method. Competencies are the product meaning. A completed competency must be implemented vertically across database, backend, API, frontend, tests, audit, and documentation.

Do not treat a tracer bullet as a simple implementation ticket. A tracer bullet is complete only when it proves that Luna can perform a coherent responsibility for the household.

## Competency Lifecycle

Every Luna competency follows this lifecycle:

1. Observe.
2. Understand.
3. Prepare Work.
4. Authority Check.
5. Approval if required.
6. Execute allowed internal action.
7. Audit.

External execution is not part of the current MVP. Luna may prepare work, coordinate internal records, and execute allowed internal actions. Luna must not perform payments, banking actions, MYOB writes, email sends, legal filing, insurance renewals, bookings, cancellations, or other external consequential actions until the platform reaches the required maturity level and the household authority contract explicitly permits it.

## Completed Competencies

### 1. Bill Approval

Luna can prepare a bill for approval.

Capability:

- Uploading a bill creates a source document and bill record.
- Luna prepares bill work through a work order.
- Approval supports approve, reject, and dismiss.
- Decisions are routed through work orders, approval requests, and audit.
- No external payment or banking action occurs.

Product meaning:

Luna has learned to receive a household bill, extract useful information, prepare the work for a human decision, and record the decision trail.

### 2. Document Filing

Luna can prepare a document filing suggestion.

Capability:

- Luna extracts document text and metadata.
- Luna classifies the document enough to suggest a cabinet path.
- Cabinet filing is authority-governed.
- Original documents remain untouched unless approved.
- Reject or dismiss sends the document to `needs_review`.
- Bill ingest routes cabinet suggestions through the same approval workflow.
- Filing suggestions include evidence, reasoning, approval, and audit.

Product meaning:

Luna has learned to organize household records like a careful employee: propose where a document belongs, ask before finalizing the filing decision, preserve the source document, and keep an audit trail.

### 3. Obligation Monitoring

Luna learns that documents and bills can create household obligations.

Capability:

- Confirmed bills create or update obligations.
- Due dates create reminders.
- Workbench shows upcoming obligations, overdue obligations, and needs-review obligations.
- Audit records obligation creation and status changes.
- No external payment, banking, email, or provider action occurs.

Product meaning:

Luna begins to monitor household responsibilities over time, not merely store records. It should help the household know what is due, what is overdue, and what needs review.

Implementation note:

- Existing confirmed bills can be backfilled through the internal `luna.obligations.backfill_confirmed_bills` worker task.
- Routine obligation maintenance can run through the internal `luna.obligations.maintenance` worker task.
- The Workbench read path currently calls a clearly named prototype boundary, `refresh_overdue_obligations_for_workbench_read`, so overdue status stays visible before scheduled workers are operational.
- Read-triggered overdue refresh is temporary prototype behavior. It should be removed once internal scheduled maintenance is reliably running.
- No external integrations or external execution are part of this competency.

## Next Recommended Competency

### 4. Household Graph Relationship Suggestions

Luna learns to suggest household graph relationships for approval.

Target capability:

- Bill or document evidence creates relationship suggestions.
- Suggestions include affected entities, reasoning, confidence, and source records.
- Work orders and approvals govern graph updates.
- Accepted suggestions update the graph.
- Rejected or dismissed suggestions remain auditable.

## Design Review Requirement

After implementing each competency, Codex must produce a design review covering:

1. Technical debt introduced.
2. Reusable abstractions created.
3. Tests added.
4. Risks.
5. Recommended cleanup before the next competency.
6. Whether the competency is prototype-quality or production-quality.

This review is required before starting the next competency.
