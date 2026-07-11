# Principles

1. Luna is an AI household employee, not a bill tracker, budgeting app, document vault, personal finance app, or generic chatbot.
2. The household is the employer organization; the main household administrator delegates authority to Luna and to other household members.
3. Luna sits above existing systems instead of replacing banks, MYOB, calendars, email, accountants, insurance portals, government services, utilities, or cloud storage.
4. Authority is a product primitive: every read, write, and execute capability must be governed by role, permission, scope, approval, spending limit, and audit trail.
5. The household knowledge graph is the core memory and context asset.
6. Every important record should be connected to people, places, accounts, assets, deadlines, obligations, authority, and documents where possible.
7. Original documents remain the source of truth; extracted data is structured interpretation.
8. AI should be proactive, not only reactive.
9. AI output is draft work until trusted by rules, confidence thresholds, delegated authority, or human review.
10. Humans approve financial, legal, filing, booking, cancellation, renewal, and other consequential actions unless a specific trusted routine delegation exists.
11. The MVP must stay focused enough to ship.
12. Security, privacy, auditability, and data ownership are product requirements, not later polish.
13. User-owned storage is the default; Luna-managed cloud storage is optional.
14. The household cabinet should be generated from the user's graph, not forced into one fixed folder structure.
15. A feature belongs in Luna only if a competent household employee would reasonably be expected to perform that responsibility.

## Product Posture

Luna should reduce administrative load without hiding important decisions from the household administrator. It can extract, classify, connect, summarize, remind, prepare actions, communicate, write drafts, and later execute approved routine work. It should avoid pretending to be the bank, accountant, school, insurer, or government portal. Instead, Luna coordinates across those systems and gives the household a single employee-like interface for asking, "What do we know, what is due, what needs approval, and what should happen next?"

Luna should also avoid asking users to manually organize every file. It should maintain a household cabinet that is understandable in normal folders, while the richer graph tracks every relationship behind the scenes.

## Engineering Posture

Build narrow workflows on broad foundations. Phase 1 can focus on records, documents, bills, obligations, and assisted work, but the system should use names, tables, services, and APIs that can grow into household members, authority contracts, relationship mapping, integrations, approvals, reminders, tasks, work orders, and multi-user access.
