---
status: accepted
---

# Make Conversation the primary household interface

> **MVP scope note:** ADR 0019 retains Conversation as the primary interface but supersedes the assumption that Document Handling is the durable product centre. New work must use Conversation to explain and direct Household Work across email, documents and other sources.

Household members communicate with Luna through natural conversation rather than software workflows. Document Handling, Household Context, authority, Filing Decisions, Filing Rules, execution state and Audit History remain typed and durable in their owning domains; Conversation derives prompts from that state, interprets a member utterance into candidate Member Direction, validates it, and submits commands to those domains.

Luna asks for only the next materially necessary answer and hides structured state by default. Minimal inline actions may support a conversational prompt, while natural-language replies remain available. Extracted evidence can inform a proposal but cannot grant authority or execute a consequential action.

When local evidence is complete enough to propose both Household Context and a Filing Decision, Luna presents that proposal as one confirmation. The member's affirmative reply confirms the proposed context and authorises that filing; the extracted evidence alone still does neither. Incomplete evidence continues as one focused clarification at a time.

Structured review and correction remain available through an optional **Review details** surface for transparency, support and development. This surface is not the default path and does not become a second owner of Document Handling state.

Member-direction interpretation is a replaceable boundary. The installed local-first application uses a deterministic interpreter for supported replies, tests may provide deterministic implementations, and a future local or consent-gated Intelligence Provider may propose interpretations. Every interpretation is still validated by Luna-owned domain rules before state changes.

The first vertical slice continues to operate in the authenticated Household Organiser session already used by the desktop application. Conversation Orchestration does not create a separate authority path: accepted utterances enter the same Member Direction and Filing Decision commands and produce the same audit authority as structured review. Future multi-member identity support belongs on those domain commands, not inside the interpreter.

This decision preserves recovery and auditability because deleting a Conversation can remove its dialogue without deleting the related Document Handling, Filing Rule or Audit Events. It also means conversational copy and interpretation can evolve without migrating durable work state.
