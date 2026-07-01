# AI Agents

Luna uses AI to reduce administrative work, but it should not take irreversible financial actions without human approval.

## MVP Agent Responsibilities

- Extract structured bill and invoice fields from documents.
- Suggest categories such as utilities, insurance, rates, registration, rent, tax, repairs, or subscriptions.
- Suggest classification as personal, business, or property-related.
- Flag missing or low-confidence data for human review.

## Provider-Agnostic Interface

The backend should call an internal extraction interface rather than a provider directly. This allows Luna to swap OpenAI, Claude, Gemini, or another provider later.

## Safety Boundaries

- AI may prepare actions, but humans approve financial actions.
- AI output is treated as draft data until reviewed or accepted by a user.
- Low confidence or conflicting values must be surfaced instead of silently accepted.
- Prompts and extraction outputs should avoid storing unnecessary personal data.
