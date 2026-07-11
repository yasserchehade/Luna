# Platform Maturity

Luna should mature deliberately. Higher maturity levels expand what Luna can safely do, but they do not remove the need for authority, approval, provenance, and audit.

## Level 1 - Single-Household Prototype

One household, one local development environment, and a narrow set of household records.

Luna may:

- Store source documents.
- Extract and structure data.
- Prepare work for approval.
- Show Workbench items.

Luna must not:

- Execute external actions.
- Assume multi-household isolation is production-ready.
- Treat AI output as trusted without review.

## Level 2 - Authority and Approval Layer

Luna has explicit work orders, approval requests, connection scopes, and authority policies.

Luna may:

- Route internal record changes through work orders.
- Request approval before sensitive internal changes.
- Record approve, reject, and dismiss decisions.

Luna must not:

- Bypass authority for filing, financial, legal, or irreversible actions.
- Treat a UI button as approval unless it creates an auditable decision.

## Level 3 - Audit and Provenance

Luna can explain what changed, why it changed, who approved it, and which source records supported it.

Luna may:

- Show audit history for household work.
- Link decisions to source documents, bills, entities, and work orders.
- Preserve rejected or dismissed suggestions for review.

Luna must not:

- Lose the original source document.
- Overwrite evidence without provenance.

## Level 4 - Read-Only Integrations

Luna can observe approved external services without acting in them.

Examples:

- Email read access.
- Calendar read access.
- Bank transaction read access.
- Insurance portal read access.
- Cloud storage read access.

Luna may:

- Retrieve records.
- Detect obligations and risks.
- Prepare internal work from observed evidence.

Luna must not:

- Send emails.
- Pay bills.
- Modify accounts.
- Renew, cancel, book, file, or submit externally.

## Level 5 - Specialist Internal Agents

Luna remains one public employee interface while internal specialist services handle domains.

Examples:

- Finance specialist.
- Documents specialist.
- Property specialist.
- Vehicle specialist.
- Insurance specialist.
- Scheduling specialist.
- Communications specialist.
- Compliance/government specialist.

Luna may:

- Route work internally.
- Combine specialist outputs into one Workbench.
- Keep specialist reasoning grounded in records and authority.

Luna must not:

- Expose internal specialists as separate household employees.
- Let specialists bypass the authority contract.

## Level 6 - Trusted Internal Automation

Luna can perform low-risk internal work automatically within explicitly delegated authority.

Examples:

- Normalize internal metadata.
- Create reminders for confirmed obligations.
- Update internal statuses from confirmed evidence.
- Prepare recurring internal reports.

Luna may:

- Execute allowed internal actions without per-action approval.
- Escalate exceptions.
- Keep a correction path.
- Run internal scheduled maintenance such as obligation backfills and overdue-status refresh.

Luna must not:

- Perform external consequential actions.
- Automate work without confidence thresholds, rollback/correction paths, and audit.

Prototype note:

Until scheduled internal workers are reliable, Luna may temporarily refresh overdue obligation status during Workbench reads through a clearly named service boundary. This is acceptable only as prototype behavior. Production-quality obligation monitoring should run through internal scheduled work, not through user reads.

## Level 7 - External Authorised Execution

Luna can perform approved real-world actions in external systems.

Examples:

- Paying bills.
- Booking services.
- Renewing policies.
- Cancelling subscriptions.
- Sending emails.
- Filing forms.
- Writing to accounting systems such as MYOB.

This level is out of current MVP scope.

Before Level 7, Luna needs mature authority contracts, production security review, real integration safeguards, error recovery, legal/compliance review, household role management, approval limits, and clear escalation rules.
