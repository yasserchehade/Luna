# Project Luna PRD

## 1. Product Summary

Project Luna is an AI household employee for modern household administration.

The household is the employer organization. One household member is the main household administrator and leader, usually the subscriber. That person hires Luna, creates the household profile, invites other household members, assigns authority levels, and decides what Luna may observe, prepare, write, or execute.

Luna does not replace banks, email, calendars, insurance portals, accountants, MYOB, government services, cloud storage, school systems, utility providers, or other household services. Luna connects to those systems, retrieves relevant information, organizes records, monitors obligations, prepares decisions, and coordinates approved work on behalf of the household.

The MVP must prove Luna as a useful assisted household operations employee without overbuilding autonomous execution.

## 2. Mission

Eliminate repetitive household administrative work that does not add value to the family's life.

## 3. Product Principles

- Luna is an AI household employee, not a bill tracker, budgeting app, document vault, personal finance app, or generic chatbot.
- The household is the employer organization.
- Authority is central. Luna's work depends on the household employment contract.
- Luna is public-facing as one employee, even if specialist agents perform work internally.
- Luna connects to existing services instead of replacing them.
- Original documents remain the source of truth.
- Extracted data and AI output are draft work until validated, accepted, or authorised.
- Sensitive or consequential work requires approval unless explicitly delegated.
- User-owned storage is the default.
- A feature belongs in Luna only if a competent household employee would reasonably be expected to perform it.

## 4. Target Users

### Primary User

Household administrator:

- Usually the subscriber.
- Owns household setup, billing, member invitations, and authority rules.
- Reviews Luna's prepared work.
- Approves or rejects consequential actions.

### Secondary Users

Household members:

- May upload records, view permitted information, answer questions, or approve work within assigned authority.
- May have limited access based on household role.

### Future External Users

External professionals or helpers:

- Accountants, bookkeepers, property managers, family assistants, or service providers.
- Access must be scoped, auditable, and controlled by the household administrator.

## 5. Problem Statement

Household administration is scattered across bills, documents, emails, cloud folders, calendars, service providers, banks, accounting tools, government portals, insurers, utilities, and paper records.

Families waste time remembering where information lives, what is due, what has been paid, which provider needs follow-up, what documents are missing, and what should happen next.

Existing tools solve narrow slices. Luna should coordinate across them as a household operations employee.

## 6. MVP Goal

Build an assisted-work MVP focused on household records, bills, documents, obligations, authority levels, and approval workflows.

The MVP should let Luna:

- Maintain a basic household profile.
- Represent household members and simple roles.
- Accept document and bill uploads.
- Extract key bill and document information.
- Link records to household entities.
- Create tasks and reminders.
- Prepare work for review.
- Request approval when authority requires it.
- Show what needs attention in a household operations dashboard.
- Preserve evidence, confidence, provenance, and audit history.

The MVP must not execute payments, bookings, renewals, cancellations, legal filings, external messages, or irreversible changes.

## 7. Non-Goals

The MVP will not include:

- Autonomous bill payment.
- Production-grade banking integrations.
- Production-grade MYOB/accounting integrations.
- Real email inbox automation.
- External service write or execute actions.
- Full multi-household SaaS billing operations.
- Production-grade authentication and identity management.
- Mobile apps.
- Tax filing, insurance renewal, or government lodgement execution.
- General-purpose chatbot behaviour unrelated to household operations.

## 8. Authority Model

Luna's operating boundary is the household employment contract.

The contract defines:

- What Luna may read.
- What Luna may write.
- What Luna may execute.
- What requires approval.
- Who can approve.
- Spending limits.
- Escalation rules.
- Which members or external parties can access which information.
- Which work types are observation-only, assisted, or trusted routine work.

### Connection Levels

1. Read: Luna can observe and retrieve information.
2. Write: Luna can create or modify non-final records such as drafts, calendar holds, folders, tasks, notes, or prepared messages.
3. Execute: Luna can perform consequential actions such as paying bills, booking services, renewing policies, lodging forms, cancelling subscriptions, or sending messages externally.

Execution is never implied by read or write access.

### Initial Roles

- Owner: manages the household contract, members, integrations, billing, and all approvals.
- Admin: manages records and approves routine work within owner-defined limits.
- Member: views and contributes records relevant to their authority.
- Viewer: views permitted records only.

## 9. Work States

Luna's work should move through explicit states:

- Observed: Luna has seen information but has not acted.
- Prepared: Luna has extracted, organized, drafted, or matched information.
- Proposed: Luna recommends a next step.
- Approval requested: Luna needs a permitted person to approve.
- Approved: the action has been approved within the authority contract.
- Executed: Luna or an integration completed the action.
- Escalated: Luna could not safely proceed and needs human attention.
- Rejected: the household declined the proposed action.
- Dismissed: the household closed the item without action.

MVP work should stop at prepared, proposed, approval requested, approved, rejected, dismissed, or escalated.

## 10. Core MVP Requirements

### 10.1 Household Profile and Members

Requirements:

- Create or seed a household profile.
- Identify the main household administrator.
- Support household member records.
- Support initial roles: owner, admin, member, viewer.
- Record role changes in audit history.

Acceptance criteria:

- A household has one owner.
- Each member has a role.
- Future authority checks can reference household, member, and role.

### 10.2 Authority Policies

Requirements:

- Represent authority policies for work type, capability level, approver role, spending limit, and escalation rule.
- Determine whether Luna may prepare, request approval, or block a proposed action.
- Prevent execute actions in MVP.

Acceptance criteria:

- A proposed consequential action can be classified as allowed, approval required, blocked, or escalation required.
- The authority decision is recorded with reason metadata.

### 10.3 Documents and Records

Requirements:

- Upload household documents, especially PDF bills and invoices.
- Store original documents in user-owned cabinet storage.
- Extract document text where possible.
- Preserve original filename, source, storage provider, storage path, hash, and received timestamp.
- Generate suggested cabinet paths from household graph context.
- Let authorised users confirm or reject cabinet filing suggestions.

Acceptance criteria:

- Uploaded documents are stored outside the web root.
- Luna can show document status: unplanned, suggested, confirmed, filed, or needs review.
- Filing suggestions require confirmation before file movement or irreversible changes.

### 10.4 Bills and Obligations

Requirements:

- Extract supplier, amount, currency, due date, invoice number, category, classification, confidence, and review reasons.
- Store extracted bills as draft or reviewable obligations.
- Link bills to source documents.
- Link bills to supplier entities when known.
- Create reminders before due dates.
- Create review tasks for missing, conflicting, low-confidence, or changed-template results.

Acceptance criteria:

- A bill can be draft, unpaid, paid, overdue, or archived.
- A bill has review status: needs review, confirmed, or not required.
- Low-confidence extraction creates a review task.
- Due dates create reminders.

### 10.5 Household Knowledge Graph

Requirements:

- Represent household entities such as household, family member, supplier, property, business, utility account, subscription, document, bill, task, and reminder.
- Represent relationships between entities with provenance and confidence where available.
- Generate suggestions for creating entities, connecting entities, attaching documents, updating metadata, or merging duplicates.
- Let authorised users accept or reject graph suggestions.

Acceptance criteria:

- A document or bill can be linked to at least one household entity.
- Each AI-suggested relationship includes reasoning, confidence, and source reference where possible.
- Rejected suggestions remain auditable.

### 10.6 Assisted Work and Approval Requests

Requirements:

- Create work records for Luna-prepared actions.
- Create approval requests when Luna proposes consequential work.
- Link approval requests to source documents, bills, tasks, or entities.
- Show evidence, confidence, requested approver role, and reason for approval.
- Let authorised users approve, reject, dismiss, or escalate.

Acceptance criteria:

- Luna can say what it prepared, why it prepared it, what evidence it used, and what approval is needed.
- Approval decisions are recorded in audit history.
- No approved work executes externally in MVP.

### 10.7 Dashboard

Requirements:

- Show a household operations dashboard, not just a bill list.
- Highlight needs attention, approvals requested, risks detected, upcoming obligations, overdue items, and prepared work.
- Keep dashboard separate from cabinet archive and structure setup.
- Provide clear next actions for review, approve, correct, dismiss, or archive.

Acceptance criteria:

- A user can see what Luna needs from them today.
- Approval requests are visible alongside tasks and reminders.
- The dashboard language reinforces Luna as an employee preparing work.

### 10.8 Luna Interface

Requirements:

- Provide one public-facing Luna interface.
- Ground answers in structured records and source documents.
- Show sources and confidence where possible.
- Suggest next actions only when grounded in household records or authority state.
- Avoid generic chatbot behaviour.

Acceptance criteria:

- Luna answers household questions such as "What is due soon?", "What needs approval?", "Which bills need review?", and "Where is this document?"
- Answers cite relevant records or documents.
- Luna does not speculate when evidence is missing.

### 10.9 Audit and Provenance

Requirements:

- Record important events for documents, bills, graph suggestions, tasks, reminders, approvals, authority decisions, and Luna answers.
- Avoid logging sensitive document contents.
- Preserve provider, model, confidence, and source evidence for AI extraction or suggestions where available.

Acceptance criteria:

- A user can inspect who confirmed, corrected, approved, rejected, dismissed, or escalated work.
- AI-created data can be traced back to source evidence.

## 11. Onboarding Requirements

Luna onboarding should feel like onboarding a household employee, not configuring software.

Lifecycle:

1. Recruitment: household hires Luna and creates the household profile.
2. Orientation: Luna learns members, roles, assets, services, records, and responsibilities.
3. Observation: Luna watches patterns and builds understanding before taking action.
4. Assisted Work: Luna suggests actions, prepares work, and asks for approval.
5. Trusted Employee: Luna executes authorised routine work and escalates exceptions.

MVP onboarding should cover recruitment, basic orientation, and assisted work. Observation can be simulated through uploads until read-only integrations exist.

## 12. UX Requirements

Primary navigation should evolve toward:

- Workbench: what Luna needs from the household now.
- Records: household documents and cabinet.
- Obligations: bills, invoices, due dates, renewals, and reminders.
- Household: members, roles, entities, properties, suppliers, and relationships.
- Approvals: work awaiting authorised decision.
- Luna: grounded question and answer interface.
- Audit: history of important records, decisions, and actions.
- Settings: storage, authority, and future connections.

Language guidelines:

- Prefer "Luna prepared", "needs approval", "assigned authority", "household records", and "work awaiting review".
- Avoid making the product feel like a generic AI chat surface.
- Avoid making bills the entire product surface.

## 13. Data Model Requirements

Near-term schema additions should include:

- `authority_policies`
- `approval_requests`
- `work_orders`
- `connection_scopes`
- Authority decision metadata on audit events.

Existing concepts to keep:

- `workspaces`
- `users`
- `workspace_memberships`
- `household_entities`
- `entity_relationships`
- `documents`
- `document_texts`
- `bills`
- `tasks`
- `reminders`
- `graph_suggestions`
- `extraction_runs`
- `audit_events`

## 14. Technical Requirements

- Backend remains FastAPI.
- Frontend remains Next.js and React.
- PostgreSQL remains the structured source of truth.
- Redis and Celery remain the job infrastructure.
- AI calls remain behind provider-agnostic service interfaces.
- Route handlers should orchestrate work, not contain provider-specific AI or authority logic.
- Every future write or execute integration must call the authority layer first.
- Original documents should remain immutable where possible.
- User-owned storage remains the default.

## 15. Security and Privacy Requirements

- Household data must be isolated by household/workspace.
- Uploaded files must be validated by type and size.
- Secrets must remain outside source control.
- Integration scopes must request the minimum useful access.
- Read, write, and execute permissions must be separate.
- Execute actions require explicit authority and audit trails.
- AI output must be treated as draft work until validated or accepted.
- Sensitive actions must be blocked or escalated when authority is unclear.
- Audit logs must avoid storing sensitive document contents.

## 16. Success Metrics

MVP success should be measured by:

- Number of household records uploaded and organized.
- Percentage of bills extracted with high confidence.
- Percentage of low-confidence items routed to review.
- Number of reminders created from obligations.
- Number of graph links created or confirmed.
- Number of approval requests created and resolved.
- Time from upload to prepared work.
- Reduction in unreviewed or overdue obligations.
- User trust indicators: corrections, rejected suggestions, and repeated use.

## 17. Release Milestones

### Milestone 1: Product Foundation

- PRD, vision, authority, and roadmap docs aligned.
- Current bill/document MVP audited against the AI household employee model.

### Milestone 2: Authority-Aware Data Model

- Add authority policies.
- Add work orders.
- Add approval requests.
- Extend audit events for authority decisions.

### Milestone 3: Assisted Bill Workflow

- Link extracted bills to work orders.
- Create approval requests for consequential proposed actions.
- Show approvals on dashboard.
- Preserve evidence and confidence.

### Milestone 4: Household Workbench UX

- Reframe dashboard around Luna's prepared work.
- Add approval, review, risk, and upcoming obligation sections.
- Update product language away from generic assistant and bill tracker framing.

### Milestone 5: Read-Only Observation

- Add connection registry.
- Add read-only integration scaffolding.
- Detect obligations from connected sources without write or execute actions.

## 18. Open Questions

- What is the first household role model: workspace roles only, or separate household authority roles immediately?
- Should `work_orders` wrap all tasks and approvals, or only Luna-prepared actions?
- Which approval actions should be visible in the first UI slice?
- Should cabinet filing be treated as write work or internal assisted work in MVP?
- What is the first read-only integration after uploads: email, calendar, cloud folder, bank feed, or MYOB?
- What language should replace "Assistant" in the navigation: Luna, Work with Luna, Ask Luna, or Employee?

## 19. Decision

The next implementation step should be a minimal authority-aware assisted-work layer, not a new external integration.

Recommended first code slice:

1. Add authority policy, work order, and approval request models and tables.
2. Add services for creating work orders and approval requests.
3. Add audit events for authority decisions.
4. Link bill review and cabinet filing suggestions to work states.
5. Surface approvals and prepared work in the dashboard.

This keeps the MVP narrow while making Luna's employment contract concrete.
