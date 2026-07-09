# Authority Model

Luna is an AI household employee. Authority is therefore not a settings page detail; it is the contract that makes Luna safe and useful.

The household is the employer organization. One household member is the main household administrator and leader, usually the subscriber. That person invites other household members, external helpers, or professional contacts and decides what each person can see, approve, or delegate.

## Authority Contract

The household employment contract defines Luna's operating boundaries:

- What Luna may read.
- What Luna may write.
- What Luna may execute.
- What requires approval.
- Who can approve.
- Spending limits.
- Escalation rules.
- Which household members or external parties can access which information.
- Which work types are observation-only, assisted, or trusted routine work.

The contract should be explicit enough that every proposed action can be classified before Luna touches an external system or household record.

## Connection Levels

External service connections use three levels:

1. Read: Luna can observe and retrieve information.
2. Write: Luna can create or modify non-final records such as drafts, calendar holds, folders, tasks, notes, or prepared messages.
3. Execute: Luna can perform consequential actions such as paying bills, booking services, renewing policies, lodging forms, cancelling subscriptions, or sending messages externally.

Execution is never implied by read or write access. Execute permissions must be granted for a specific service, work type, approver group, spending limit, and escalation rule.

## Work States

Luna's work should move through clear states:

- Observed: Luna has seen information but has not acted.
- Prepared: Luna has extracted, organized, drafted, or matched information.
- Proposed: Luna recommends a next step.
- Approval requested: Luna needs a permitted person to approve.
- Approved: the action has been approved within the contract.
- Executed: Luna or an integration completed the action.
- Escalated: Luna could not safely proceed and needs human attention.
- Rejected or dismissed: the household decided not to proceed.

MVP work should stop at prepared, proposed, approval requested, approved, or dismissed. Execution can be represented in the model before it is enabled in product.

## Household Roles

Initial roles can stay simple:

- Owner: manages the household contract, integrations, billing, members, and all approvals.
- Admin: can manage records, approve routine work, and invite members within owner-defined limits.
- Member: can view and contribute records relevant to their authority.
- Viewer: can view permitted records only.

Later roles may include external accountant, property manager, bookkeeper, child account, or limited service-provider access.

## MVP Scope

The MVP should implement authority as assisted workflow, not autonomous execution:

- Store household and member roles.
- Record whether a proposed action requires approval.
- Show why Luna is asking for approval.
- Keep source evidence attached to the request.
- Record who approved, corrected, dismissed, or archived the work.
- Prevent payments, bookings, renewals, cancellations, filings, deletions, or external sends from executing automatically.

## Engineering Implications

Before adding write or execute integrations, Luna needs first-class concepts for:

- Household member roles.
- Connection capability levels.
- Authority policies.
- Approval requests.
- Work orders.
- Spending limits and escalation rules.
- Audit events tied to proposed and completed actions.

These should sit below individual features such as bills, documents, calendar events, bank reconciliation, and insurance renewals. A bill payment should be a type of work governed by authority, not a special-case button.
