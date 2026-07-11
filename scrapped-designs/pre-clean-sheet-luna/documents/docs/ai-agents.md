# AI Agents

Luna uses AI as an understanding and coordination layer over household information. The goal is not to build a chatbot bolted onto a bill tracker. The goal is an AI household employee that reads documents, extracts structure, maps relationships, creates reminders, prepares work, requests approval, and helps the household understand what needs attention.

AI output should be grounded in Luna's documents, structured database records, authority contract, and integration data. Sensitive actions require authority checks and human approval unless a specific trusted routine delegation exists.

Luna is public-facing as one employee. Internally, specialist agents may perform work, but the household should not need to know those specialists exist.

## Provider-Agnostic Interface

The backend should call internal AI service interfaces rather than provider SDKs directly from route handlers or workers. This allows Luna to swap or combine OpenAI, Claude, Gemini, local models, or specialist extraction services later.

Core interfaces should eventually cover:

- Document understanding.
- Entity extraction.
- Classification.
- Relationship mapping.
- Summarization.
- Retrieval-grounded assistant responses.
- Briefing generation.
- Authority-aware work preparation.
- Approval request drafting.

## Core Agents

### Intake Agent

Receives uploaded, emailed, synced, or imported source material. It identifies document type, source system, duplicates, priority, and the next processing step.

### Authority Agent

Checks whether Luna may read, write, or execute a proposed action. It evaluates the household contract, member roles, connection scopes, spending limits, approval rules, and escalation rules. It should be called before any write or execute action and before surfacing an approval request as ready.

### Document Understanding Agent

Extracts text and structure from PDFs, images, emails, and other files. It identifies tables, totals, dates, account numbers, issuer details, and source evidence.

### Supplier Template Agent

Maintains supplier-specific extraction profiles. When Luna sees a known supplier, it checks expected anchors, records a template fingerprint, and detects whether the document matches a known template or appears to have changed. If anchors are missing or confidence drops, Luna should fall back to generic extraction and create a review task.

### Entity Extraction Agent

Turns unstructured content into typed entities such as Supplier, Bill, Invoice, Property, Vehicle, InsurancePolicy, UtilityAccount, Subscription, SchoolObligation, HealthEvent, GovernmentDocument, TaxObligation, Warranty, or Asset.

### Relationship Mapping Agent

Connects entities together. For example, it can infer that a water bill belongs to a property, a payment settles an invoice, a policy covers a vehicle, or a warranty belongs to an appliance.

### Reminder Agent

Creates reminders from due dates, renewal dates, appointment times, expiry dates, and missing information. It should explain what triggered each reminder.

### Finance Agent

Understands bills, invoices, payments, subscriptions, bank account context, MYOB/accounting context, accounts payable, accounts receivable, routine reconciliation, and reporting inputs. It may prepare payment, reconciliation, invoice-processing, or export suggestions, but users approve financial actions unless covered by an explicit trusted routine delegation.

### Property Agent

Connects property-related bills, rates, insurance, utilities, repairs, warranties, documents, and tax-relevant expenses.

### Vehicle Agent

Tracks registration, insurance, servicing, warranties, fines, documents, and vehicle-related reminders.

### Family Calendar Agent

Connects school events, health appointments, family obligations, reminders, and external calendars into a household schedule.

### Knowledge Agent

Answers natural-language questions using structured records and indexed source documents. It should cite or link back to source records wherever possible.

The first implementation is deterministic and grounded in Luna records rather than a general LLM. It maps simple questions to structured data such as bills, tasks, reminders, documents, and cabinet search results. Model-backed reasoning can be added behind the same interface later.

### Daily Briefing Agent

Produces proactive daily or weekly summaries: what is due, what changed, what needs review, what was paid, what is overdue, and what the household should decide next.

### Communications Agent

Prepares drafts for service providers, insurers, schools, accountants, government agencies, or household members. Sending external communications is an execute action unless the authority contract explicitly allows the specific routine send.

### Compliance and Government Agent

Tracks government, tax, identity, renewal, and compliance-related obligations. It can organize records and prepare drafts, but filing or submitting forms requires explicit authority and approval.

## Safety Boundaries

- AI may prepare actions, but humans approve financial, legal, filing, booking, renewal, cancellation, external-send, or irreversible actions unless a narrow trusted routine delegation exists.
- AI output is draft data until reviewed, accepted, or validated by deterministic rules.
- Low-confidence or conflicting values must be surfaced instead of silently accepted.
- Prompts and outputs should avoid storing unnecessary personal data.
- Assistant answers should be grounded in Luna records and documents.
- Every automated relationship or reminder should retain provenance where possible.
- Specialist agents remain internal implementation details; the product surface is Luna.
