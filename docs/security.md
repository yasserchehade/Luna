# Security

Security is mandatory from day one because Luna handles household documents, financial records, identity-adjacent information, family schedules, school and health context, business records, and government or tax material.

Luna should be designed as a trusted household employee. That means privacy, access control, authority, auditability, and user approval are part of the product, not separate compliance chores.

## MVP Security Requirements

- Keep secrets in environment variables, never in source control.
- Store original documents in user-owned cabinet storage outside the web root.
- Validate uploaded file types and sizes.
- Log important actions without logging sensitive document contents.
- Require explicit user approval before financial, legal, filing, booking, renewal, cancellation, external-send, or irreversible actions unless the household authority contract explicitly delegates that routine work.
- Design database records around household or workspace ownership and access boundaries.
- Treat AI output as untrusted draft data until validated or accepted.
- Preserve provenance for extracted fields and automated relationships.
- Treat read, write, and execute permissions as separate capabilities.
- Record why Luna believed an action was allowed, blocked, or escalated.

## Data Boundaries

- Household data must be isolated from other households.
- User roles should eventually distinguish owner, admin, member, and viewer access.
- The owner should control the household authority contract, including member roles, connection scopes, spending limits, approval rules, and external access.
- Children, school, health, identity, and government records should be handled with extra care.
- Integrations should request the minimum useful scopes.
- Integrations must declare whether a permission is read, write, or execute.
- Source documents should remain immutable where possible, with corrections stored as structured records.
- Original household files should stay local or in user-owned cloud storage by default.
- Luna-managed cloud document storage should be opt-in and clearly described.
- Luna should not silently delete, overwrite, or irreversibly move files.

## AI and Integration Controls

- Route AI calls through internal services so provider-specific privacy controls are centralized.
- Avoid sending unnecessary context to AI providers.
- Record which provider and model produced an extraction or suggestion.
- Make automated links explainable through confidence, provenance, and source evidence.
- Never let an integration perform payment, filing, deletion, booking, renewal, cancellation, external communication, or account changes without an authority check and explicit approval where required.

## Future Requirements

- Authentication and household roles.
- Authority policies, approval requests, connection scopes, spending limits, and escalation rules.
- Encryption at rest for files.
- Encryption at rest for any Luna-managed cloud document storage.
- Signed URLs for document access.
- Audit logs for document views, edits, exports, AI suggestions, and integration syncs.
- Data retention, deletion, and export policies.
- Provider-specific controls for AI data handling.
- Secrets management for production integrations.
- Backup and restore strategy for structured records and optional cabinet backups.
