# Tracer Bullet Plan

Every Luna implementation issue should be a vertical tracer bullet. Each bullet delivers one household-employee responsibility end to end and includes acceptance criteria.

## Tracer Bullet 1: Luna Prepares a Bill for Approval

Goal: prove Luna's employee model end to end.

User story: as a household administrator, I want Luna to receive a bill, extract the important information, prepare it as work, request approval where required, and record my decision, so that household obligations are managed safely and transparently.

Flow:

1. Upload bill.
2. Extract bill data.
3. Create bill record.
4. Create work order with evidence, confidence, source document, and status.
5. Run authority decision.
6. Create approval request if required.
7. Show request in Workbench.
8. User approves, rejects, or dismisses.
9. Audit records the decision trail.
10. No external action occurs.

Scope:

- Bill upload and extraction.
- Work order creation.
- Authority decision.
- Approval request.
- Workbench visibility.
- Approve, reject, and dismiss decisions.
- Audit events.

Out of scope:

- Payment.
- Bank integration.
- MYOB integration.
- Email integration.
- External execution.
- Autonomous approval.

Acceptance criteria:

- Uploading a bill creates a bill record.
- Luna creates a work order for the prepared bill.
- Authority service determines whether approval is required.
- Approval request is visible in Workbench.
- User can approve, reject, or dismiss.
- Audit records the full decision trail.
- No external action occurs.

## Tracer Bullet 2: Luna Prepares a Document Filing Suggestion

Flow: upload document -> extract text and metadata -> classify document -> suggest cabinet path -> create work order -> request approval if filing changes storage or path -> user approve or reject -> file or mark confirmed -> audit.

Acceptance criteria:

- Luna never silently moves or finalizes records without authority.
- Filing suggestions include reasoning.
- Rejected suggestions remain auditable.
- User can find the original document.

## Tracer Bullet 3: Luna Turns a Bill Into an Obligation With Reminders

Flow: confirmed bill -> obligation created or updated -> due date detected -> reminder created -> appears in Workbench upcoming obligations -> overdue state if unpaid after due date -> audit.

Acceptance criteria:

- Due bills appear as obligations.
- Reminders are created automatically.
- Overdue status is clear.
- User can see what Luna needs them to do today.

## Tracer Bullet 4: Luna Suggests Household Graph Relationships for Approval

Flow: bill or document extracted -> supplier or entity detected -> relationship suggested -> work order created -> approval or rejection requested -> graph updated only after approval -> audit.

Acceptance criteria:

- Luna can suggest, but not silently finalize, new relationships.
- Accepted suggestions update the graph.
- Rejected suggestions remain in audit history.
- Household graph becomes more useful without becoming unsafe.

## Tracer Bullet 5: Luna Workbench Shows What Needs Attention Today

Flow: tasks, approvals, obligations, and risks -> normalized into Workbench items -> sorted by urgency -> user acts -> state changes -> audit.

Acceptance criteria:

- User can open Luna and immediately know what needs attention.
- Workbench language reinforces Luna as household employee.
- Bills are not the whole product surface.
- Approvals and prepared work are visible.

## Tracer Bullet 6: Luna Answers Questions Using Household Records

Flow: user asks question -> Luna searches structured records and documents -> answer generated with sources -> uncertainty shown if evidence is missing -> suggested next action if grounded -> audit answer metadata.

Acceptance criteria:

- Luna answers only from household evidence.
- Luna cites records or documents where possible.
- Luna does not speculate.
- Luna suggests next actions only when supported by data and authority.

## Tracer Bullet 7: Luna Onboards a Household Gradually

Flow: create household -> identify owner -> add members -> assign roles -> choose responsibilities -> explain required access -> seed first authority policies -> show first Workbench state.

Acceptance criteria:

- User understands Luna as an employee.
- Household owner is established.
- Members have roles.
- Luna has initial responsibilities and authority boundaries.

## Tracer Bullet 8: Luna Registers a Read-Only Connection Scope

Flow: user selects responsibility -> Luna explains required connection -> connection scope created -> status visible in settings -> no real integration required initially -> audit.

Acceptance criteria:

- User can see what Luna is allowed to observe.
- Read, write, and execute are clearly separate.
- No external execution is possible.
- Future integrations have a safe permission model.

## Tracer Bullet 9: Luna Routes Work to Specialist Services Internally

Acceptance criteria:

- User only sees Luna.
- Internally, work can be routed by domain.
- New specialists can be added without rewriting Luna's public interface.

## Tracer Bullet 10: Luna Performs Low-Risk Internal Work Within Authority

Do not build until assisted workflows are trusted.

Acceptance criteria:

- Automatic work only happens inside approved authority.
- User can inspect and correct automatic work.
- Audit explains why Luna acted automatically.
