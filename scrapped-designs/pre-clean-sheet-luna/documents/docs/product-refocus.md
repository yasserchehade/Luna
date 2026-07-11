# Product Refocus: AI Household Employee

This document records the current product refocus so future implementation work stays aligned.

## Refined Destination

Luna is an AI household employee.

The household is the employer organization. The main household administrator, usually the subscriber, hires Luna, invites household members, grants authority, and approves work when required. Luna does not replace the household's existing services. It connects to them, retrieves relevant information, organizes it, monitors obligations, prepares decisions, and coordinates approved actions.

The mission is to eliminate repetitive household administrative work that does not add value to the family's life.

## Current Repository Conflicts

The current repository is directionally compatible with this vision, but several areas still reflect the older "household OS with bill wedge" framing more than the sharper "AI household employee" model.

### Documentation

- Some docs framed Luna mostly as a household operating system or intelligence layer. That remains useful, but the primary product metaphor should now be an AI household employee.
- The previous docs mentioned approval and human review, but did not define authority as a first-class product primitive.
- Onboarding was described like product setup. It should be treated like onboarding a new household employee: recruitment, orientation, observation, assisted work, then trusted routine work.
- Specialist agents existed in the AI docs, but the docs did not clearly state that the household should interact with one Luna, not a menu of agents.

### Naming and UX

- The sidebar and page labels still include "Assistant" and "Household OS". These are not wrong for the current MVP, but the eventual surface should make Luna feel like the household's operations employee.
- The current "Create" menu is software-centric. Future language should lean toward giving Luna work, uploading records, asking for review, or approving prepared work.
- The dashboard currently summarizes bills, reminders, and tasks. It should evolve toward "Luna's workbench": needs attention, approvals requested, risks detected, work prepared, and upcoming obligations.

### Data Model

- The schema has workspaces, users, workspace memberships, household entities, documents, bills, tasks, reminders, suggestions, and audit events.
- The schema does not yet have first-class authority policies, connection scopes, approval requests, work orders, spending limits, escalation rules, or household employment contract records.
- Roles are currently workspace roles only. They need to become meaningful household authority roles before write or execute integrations are added.
- Tasks and reminders exist, but they do not yet distinguish observed information, prepared work, proposed actions, approval requests, approved work, execution, escalation, dismissal, or rejection.

### Backend Services

- Extraction, cabinet filing, graph suggestions, document search, and grounded Q&A are useful foundations.
- There is no central authority service that every write or execute action must call.
- There is no connection registry that records whether an external service grants read, write, or execute capability.
- There is no work orchestration service for Luna to prepare work, request approval, and later execute approved routine tasks.

### Product Boundary

- The current bill flow is a good wedge, but it risks becoming a bill tracker if authority, work states, and household roles do not become visible soon.
- Future features should pass the feature test: a feature belongs in Luna only if a competent household employee would reasonably be expected to perform that responsibility.

## Staged Implementation Plan

### Stage 1: Documentation and Product Language

Status: this pass updates the core docs.

- Reframe Luna as an AI household employee.
- Add the authority model and onboarding lifecycle.
- Clarify read, write, and execute connection levels.
- Clarify that specialist agents are internal implementation details.
- Keep the MVP focused on records, bills, documents, obligations, authority levels, and assisted workflows.

### Stage 2: Authority-Aware MVP Foundations

Do this before adding external write or execute integrations.

- Add household member authority fields or tables beyond generic workspace membership.
- Add an authority policy model for work type, capability level, approver role, spending limit, and escalation rule.
- Add approval request records linked to bills, documents, tasks, and future work orders.
- Add work state fields or a work order table for observed, prepared, proposed, approval requested, approved, executed, escalated, rejected, and dismissed.
- Extend audit events to capture approval, rejection, escalation, and authority decision metadata.
- Update the UI to show "Needs approval" and "Prepared by Luna" as first-class states.

### Stage 3: Assisted Bill and Document Workflows

Keep the workflow narrow and shippable.

- Upload or ingest household records and bills.
- Extract supplier, amount, due date, invoice number, category, and confidence.
- Link documents to household entities such as supplier, property, business, subscription, utility account, or family member.
- Generate cabinet path suggestions from graph context.
- Create tasks and reminders.
- Create approval requests when Luna proposes a consequential next step.
- Let authorised users confirm, correct, approve, dismiss, or archive prepared work.
- Do not execute payments, renewals, cancellations, bookings, filings, or external sends.

### Stage 4: Connection Registry and Observation

Add integrations in read-first mode.

- Model connections with provider, household, capability level, granted scopes, status, and last sync.
- Start with read access for email, calendar, cloud folder, accounting, or bank feed adapters.
- Use observation to detect bills, renewals, obligations, missing records, and unusual changes.
- Keep all write or execute actions disabled unless represented as draft or approval-requested work.

### Stage 5: Write Capabilities With Approval

Allow Luna to prepare and write non-final records.

- Draft emails, calendar holds, cabinet folders, accounting draft records, task lists, or document metadata where authorised.
- Require authority checks before every write.
- Keep a visible audit trail of what Luna wrote, why, and under whose authority.
- Provide undo or correction paths where possible.

### Stage 6: Trusted Routine Execution

Only after authority, approvals, audit, and exception handling are mature.

- Enable narrow execute permissions for routine work such as low-value recurring bill payments, approved bookings, renewals, cancellations, or accountant exports.
- Enforce spending limits and escalation rules.
- Require explicit approval for exceptions, unusual changes, low confidence, new providers, changed bank details, or sensitive categories.
- Keep the household administrator in control of delegation.

## Near-Term Architectural Direction

Before making major architectural changes, the next code pass should introduce a minimal authority layer rather than jumping directly to integrations.

Recommended first implementation slice:

1. Add `authority_policies`, `approval_requests`, and `work_orders` tables.
2. Add Pydantic models for authority policy, approval request, work order, and work state.
3. Add backend services for creating approval requests and recording authority decisions.
4. Link bill confirmation, cabinet filing, and graph suggestions to work or approval records where appropriate.
5. Update the dashboard to surface approval requests alongside tasks and reminders.

This aligns with the vision because it makes Luna's employment contract concrete while keeping the MVP grounded in household records, bills, documents, obligations, and assisted workflows.
