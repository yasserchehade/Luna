# Vision

Luna is an AI household employee.

The household is the employer organization. One household member is the main household administrator and leader, usually the subscriber. That person can invite other household members, assign authority levels, and decide what Luna may observe, prepare, write, or execute.

Luna is also the operating system for modern households: an operations platform with one AI employee interface. The user experiences Luna as a competent household operations employee, while the internal system may route work through specialist agents for finance, documents, property, vehicles, insurance, scheduling, communications, compliance, and other domains.

The mission is to eliminate repetitive household administrative work that does not add value to the family's life.

Luna connects the apps and services a family already uses, understands the relationships between their information, monitors obligations, prepares information for human decisions, and coordinates approved actions on behalf of the household.

Luna should not replace banking apps, MYOB, calendars, email, cloud storage, school apps, insurance portals, government systems, accountants, or utility portals. It should sit above them as an employee-like operations layer. The user should not need to remember whether something lives in Gmail, Google Drive, a school portal, a bank feed, MYOB, or a pile of PDFs. They should only need to ask Luna.

Luna should also avoid becoming the default cloud warehouse for every household document. The household archive should be local-first and user-owned by default. A family may keep the Luna cabinet on a local drive, external drive, NAS, iCloud Drive, Google Drive, OneDrive, Dropbox, or another preferred storage location. Luna organizes and understands the cabinet; the user owns where it lives.

## Core Promise

People should not have to remember where information is stored, which supplier needs chasing, which document is missing, or which routine administrative step comes next. They should only need to ask Luna, delegate appropriately, and approve the work that requires human authority.

## Employee Model

Luna should behave like a competent household operations employee:

- Maintain household records.
- Monitor obligations, due dates, renewals, and missing information.
- Coordinate with service providers where authorised.
- Prepare information for household decisions.
- Execute approved administrative tasks within delegated authority.
- Detect risks, missed obligations, unusual changes, and exceptions.
- Manage day-to-day accounts payable and receivable where authorised.
- Process invoices where authorised.
- Conduct routine bank and credit card reconciliations where authorised.
- Assist in compiling data for monthly and quarterly financial reports.

A feature belongs in Luna only if a competent household employee would reasonably be expected to perform that responsibility.

## Authority Contract

Authority is central. Luna's behaviour is governed by the household employment contract, which defines:

- What Luna may read.
- What Luna may write.
- What Luna may execute.
- What requires approval.
- Who can approve.
- Spending limits.
- Escalation rules.
- Which household members or external parties can access which information.

Connections to external services have three levels:

1. Read: Luna can observe and retrieve information.
2. Write: Luna can create or modify records, drafts, events, folders, or similar non-final objects.
3. Execute: Luna can perform consequential actions such as paying bills, booking services, renewing policies, or cancelling subscriptions. Execution must always be governed by the authority contract.

## Onboarding Lifecycle

Luna is not configured like ordinary software. Luna is onboarded like a new household employee:

1. Recruitment: the household hires Luna and creates the household profile.
2. Orientation: Luna learns members, roles, assets, services, records, and responsibilities.
3. Observation: Luna watches patterns and builds understanding before taking action.
4. Assisted Work: Luna suggests actions, prepares work, and asks for approval.
5. Trusted Employee: Luna executes authorised routine work and escalates exceptions.

## What Luna Becomes

Luna becomes a structured household knowledge graph, authority system, work queue, and AI employee interface. It knows about household entities, documents, deadlines, obligations, payments, relationships, authority, and context. It can answer questions, surface what needs attention, prepare the next administrative step for user approval, and later execute authorised routine work.

The graph should also drive the household cabinet. If a user models `FamilyMember -> FamilyTrust -> Property -> Supplier -> Document`, Luna can use that structure to suggest human-readable archive paths while still preserving one canonical document and many graph relationships.

It should eventually help manage:

- Bills, invoices, payment status, and subscriptions.
- Banking and MYOB/accounting context.
- Properties, vehicles, utilities, insurance, warranties, and assets.
- School obligations, health appointments, government documents, and tax deadlines.
- Family calendar events, cloud documents, reminders, and tasks.

## MVP Strategy

The MVP still starts with household records, documents, obligations, and bill processing because that workflow exercises the most important foundations:

- Household profile and main administrator.
- Member roles and initial authority levels.
- Document intake.
- Structured extraction.
- Entity assignment.
- Relationship mapping.
- Reminder creation.
- Human review.
- Approval and assisted-work state.
- Dashboard visibility.

The MVP should remain focused and buildable, but its data model and architecture must point toward the broader AI household employee from day one.

## Long-Term Direction

Luna should become a proactive employee, not a generic chatbot. It should notice upcoming obligations, connect related records, prepare action plans, coordinate routine work, and brief the family on what needs attention. It can suggest, summarize, extract, classify, coordinate, write drafts, and eventually execute authorised work, but sensitive actions such as payments, bookings, filings, renewals, cancellations, or irreversible changes require authority checks, audit trails, and explicit approval unless covered by a trusted routine delegation.
