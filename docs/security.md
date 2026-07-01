# Security

Security is mandatory from day one because Luna handles financial documents, invoices, identity-adjacent records, and business data.

## MVP Security Requirements

- Keep secrets in environment variables, never in source control.
- Store original documents outside the web root.
- Validate uploaded file types and sizes.
- Log important actions without logging sensitive document contents.
- Require explicit user approval before financial actions.
- Design database records around workspace ownership and access boundaries.

## Future Requirements

- Authentication and workspace roles.
- Encryption at rest for files.
- Signed URLs for document access.
- Audit logs for document views, edits, exports, and integrations.
- Data retention and deletion policies.
- Provider-specific controls for AI data handling.
