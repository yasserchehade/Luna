# Luna Household Administration

Luna is the domain of a digital household employee that takes ownership of household administration while leaving people in control of privacy, authority and consequential decisions. The binding product direction is in `docs/product/product-constitution.md` and `docs/product/mvp-definition.md`.

## Household identity and authority

**Luna Account**:
The minimal service-side identity that coordinates a person's Household membership and Trusted Device enrolment without granting access to cabinet contents or decryption keys.
_Avoid_: Household account, cabinet account

**External Identity**:
An authentication provider's login record mapped to a Luna Account without becoming the person's Household identity.
_Avoid_: Provider user, Supabase user

**Luna**:
The AI household employee hired to organise household administration within granted authority.
_Avoid_: Assistant, bot, generic agent

**Household**:
The employer organisation that contains members, shared context, authority and a user-owned cabinet.
_Avoid_: Workspace, tenant, account

**Household Organiser**:
The adult member who creates and administers the household, its membership and shared defaults without automatically owning another adult's private information.
_Avoid_: Owner, admin, subscriber

**Household Member**:
A person represented within a household who may hold documents, context, access and delegated responsibilities.
_Avoid_: User

**Adult Member**:
A household member who controls access to their private space and may grant authority to others.

**Dependent Member**:
A household member whose private administration is managed by a verified guardian until the applicable transition to independent control.
_Avoid_: Child account

**Guardian**:
An adult member authorised to manage a dependent member's documents and administration.
_Avoid_: Parent, unless describing the relationship rather than authority

**Delegate**:
A household member who receives authority from another member for a defined scope.

**Authority Grant**:
An explicit assignment of allowed control over a subject such as a person, property, account, document category or action.
_Avoid_: Authority transfer, permission level

**Private Space**:
The records and conversations controlled by one adult member and inaccessible to other members without an authority grant.

**Shared Space**:
Records and conversations intentionally available to authorised household members.
_Avoid_: Public space

**Trusted Device**:
A device enrolled into the household's cryptographic trust and permitted to process protected Luna state.
_Avoid_: Logged-in device

**Device PIN**:
A local secret that unlocks a Trusted Device for the current Luna session and remains separate from the Luna Account password.
_Avoid_: Account PIN, recovery PIN

**Recovery Key**:
The offline household secret that can enrol a replacement trusted device and recover encrypted Luna memory independently of account-password recovery.

**Recovery Key Replacement**:
The Household Organiser-authorised creation of a new Recovery Key from an unlocked Trusted Device, making the previous Recovery Key unusable without relying on Luna Account recovery.
_Avoid_: Recovery Key retrieval, Recovery Key reset

**Device Revocation**:
The removal of a Trusted Device's authority to receive future Household keys; after key rotation it cannot read newly protected Household state, though information it previously opened cannot be remotely erased.
_Avoid_: Remote wipe, remove device

**Portable Memory Record**:
A signed, Household-key-encrypted, append-only record of one durable household fact captured from its owning Luna behavior, stored in Luna's reserved Cabinet memory area and rebuilt into each Trusted Device's separate owning local stores.
_Avoid_: Synced database, chat memory, model memory

**Portable Memory Conflict**:
Two valid Portable Memory Records that claim incompatible current state for the same durable household subject without one causally superseding the other. Luna keeps both records and requires an explicit resolution instead of silently choosing one.

**Portable Reference**:
An opaque owning-domain identifier carried by Portable Memory. It uses an allowlisted domain kind and canonical UUID; it is not a free-text field and cannot carry prompts, provider output or credentials.
_Avoid_: Last-write-wins, sync overwrite

## Cabinet and documents

**Desk**:
Historical desktop term for the trusted-device and local-Cabinet environment. Deferred under ADR 0020; do not use for the web MVP.
_Avoid_: Workspace

**Cabinet**:
The logical household-document layer that connects sources and records to household meaning and Household Work. Luna owns meaning, relationships and logical references. A future connected user-controlled storage provider owns file bytes, versions, sharing and storage infrastructure.
_Avoid_: Vault, Luna-managed blob store, local path browser

**Incoming Cabinet Folder**:
A historical desktop folder that holds untouched Originals awaiting a Filing Decision. Deferred for the web MVP.
_Avoid_: Staging area, temporary upload folder

**Cabinet Preset**:
A historical desktop proposal for a local folder structure. Deferred for the web MVP.
_Avoid_: Fixed taxonomy

**Cabinet Availability**:
Historical desktop state describing local folder availability. A future web storage connection uses separately defined provider-connection and source-availability terms.
_Avoid_: Generic web storage health

**Document Arrival**:
One occurrence of a file entering Luna through attachment, upload, email or another intake channel.
_Avoid_: Upload

**Household Work**:
A durable piece of household administration that Luna owns from observation through understanding, member input, approval, execution, monitoring and completion, dismissal or irrelevance. It links sources, evidence, responsibility, due dates, proposed actions and outcomes.
_Avoid_: Workflow, work order, task, ticket

**Obligation**:
The internal domain term for Household Work that requires attention, a decision or an authorised action. It is not required user-facing language.

**Source**:
An email, message, document, attachment, calendar event or connected-service record that provides evidence for Household Work. A Source does not own the work lifecycle.

**Opaque Source Reference**:
A server-issued identifier that lets Household Work and conversation refer to an uploaded Source without exposing its storage key or filesystem location to a client.
_Avoid_: File path, upload path

**Document**:
The logical household record that connects one or more arrivals or versions to household context and filing decisions.
_Avoid_: File

**Original**:
The exact received bytes of a document version, preserved without content modification.
_Avoid_: Source copy

**Filed Original**:
A historical desktop state for an Original verified at a local Cabinet Destination. Future web filing terminology must reference a logical Cabinet item and provider-owned object version.
_Avoid_: Filing record, filed copy

**Document Version**:
A distinct original that updates, corrects or supersedes another original without erasing it.
_Avoid_: Duplicate

**Exact Duplicate**:
A document arrival whose bytes match an existing original.

**Possible Duplicate**:
A document arrival that appears to represent the same logical document but does not have identical bytes.
_Avoid_: Duplicate, when identity is not proven

**Service Provider**:
An external organisation that supplies a household service or issues a household document.
_Avoid_: Supplier, vendor, provider

**Addressee**:
The person or organisation to whom a document is formally directed and therefore its default responsible party.
_Avoid_: Owner

**Household Context**:
The confirmed relationship of a document to members, properties, accounts, providers and other household subjects.
_Avoid_: Metadata, graph data

**Cabinet Destination**:
Historical desktop term for a confirmed local folder and filename. Future web work uses a logical Cabinet location plus a provider-owned object reference.
_Avoid_: Cabinet path in web product language

**Filing Decision**:
A member's confirmed determination of a document's household context and cabinet destination.

**Filing Rule**:
A visible, editable rule learned from a filing decision that applies only to a declared combination of document type, service provider, addressee and household context.
_Avoid_: Model memory, automation

## Document work and attention

**Document Handling**:
The supporting lifecycle of a Document Arrival from receipt through preservation, inspection, evidence capture, clarification and filing or waiting. In the reset direction it is a source-processing capability, not Luna's central durable product domain.
_Avoid_: Product centre, task, job

**Local Inspection**:
The on-device examination of a document for type, text, checksum, duplicates and known household context without disclosing content externally.
_Avoid_: AI analysis

**Review Card**:
The structured conversational view of Luna's current understanding, evidence, uncertainties and proposed filing decision for a document.
_Avoid_: Form, approval request

**Evidence**:
The local or authorised external information supporting Luna's interpretation of a document field.

**Confidence State**:
The member-facing certainty label Confirmed, Looks right, Needs checking or Unknown.
_Avoid_: Confidence percentage

**Member Direction**:
An authorised member's answer or correction that resolves uncertainty and may teach Luna a scoped rule.
_Avoid_: Approval, when no consequential action is being authorised

**To-do Item**:
An interaction projection of Household Work that currently requires a specific household member's attention and links back to its originating sources, conversation and subject. It does not own the work lifecycle.
_Avoid_: Task, incoming item, work order

**History**:
The member-facing account of what Luna did, what people decided and which safe reversals remain available.
_Avoid_: Activity feed, audit log

**Audit Event**:
An immutable fact recording a consequential Luna or member action, its authority, subject and outcome.
_Avoid_: Log entry

## Conversation and briefing

**Today**:
The primary web home where Luna proactively briefs the member on completed work, matters requiring attention or approval and upcoming obligations, with Conversation continuously available.
_Avoid_: Dashboard, home dashboard

**Working Context**:
The concise visible context for selected Household Work: current activity, relevant source, household entity, understood facts, unresolved need and proposed action. It controls what the member is viewing, not the scope or ownership of Conversation. Detailed evidence remains behind explicit inspection.
_Avoid_: Inspector, model reasoning, debug context

**Persistent Composer**:
The global conversation input anchored to the primary workspace. It starts without requiring work selection and may show a removable Household Work, household entity or attached-source context hint without making that hint an exclusive routing target. It must not resemble a search bar.
_Avoid_: Search, command palette

**Conversation**:
A member-controlled dialogue with Luna that may contain source attachments, explanations, approvals and linked Household Work without owning the work lifecycle.
_Avoid_: Thread, chat session

**Conversation Prompt**:
Luna's typed, derived request for the next materially necessary member answer, linked to durable work without owning that work.
_Avoid_: Form step, workflow screen

**Direction Interpretation**:
A replaceable interpretation of a Member Utterance into candidate typed commands that must pass domain validation before changing state.
_Avoid_: Direct state update, authority decision

**Conversation Orchestration**:
The interaction boundary that assembles relevant Household Work and context, presents the next useful explanation or question, interprets a Member Utterance and submits validated commands to the owning domain.
_Avoid_: Work engine, conversation state machine

**Brief**:
A proactive, dated conversation in which Luna summarises completed work, relevant changes, upcoming obligations and items needing attention. `Today` presents the current Brief when the member opens Luna; production scheduling is deferred until the underlying work service is proven.
_Avoid_: Dashboard, digest

Household members communicate with Luna through natural conversation, not software workflows. Structured state exists so Luna can reason, execute and recover reliably, but it is exposed only when needed for confirmation, correction, transparency or accountability. Luna asks for the minimum materially necessary member input, one concise question at a time where practical.

## Intelligence and consent

**Intelligence Provider**:
An external or local reasoning engine used by Luna without owning Luna's household memory, authority or tools. The MVP uses Luna-managed OpenAI; local-only intelligence, BYOK and additional providers are deferred.
_Avoid_: AI, model provider, provider

**Cloud Assistance**:
Permission-gated use of an external intelligence provider when local inspection cannot safely provide enough understanding.
_Avoid_: Cloud processing

**Consent Grant**:
An explicit, revocable permission for a named intelligence provider to receive document content once or within a clearly described future scope.
_Avoid_: Do not ask again, global consent

**Luna-managed Intelligence**:
Cloud assistance supplied and supported under a Household's eligible paid plan using a tested intelligence provider, with provider usage billed to Luna.

**Household Plan**:
The service entitlement shared by a Household. A paid plan includes Luna-managed Intelligence; a free plan may use Bring-your-own Intelligence or Local-only Intelligence.
_Avoid_: Paid user, free user, subscriber

**Managed Intelligence Entitlement**:
The Household-level right to use Luna-managed Intelligence, granted through a paid Household Plan or bounded complimentary beta access. Entitlement is distinct from whether a particular Trusted Device has finished receiving access.
_Avoid_: Paid-user flag, managed credential

**Billing Subscription**:
The recurring commercial arrangement that funds an eligible Household Plan. Billing status may change entitlement but never grants Household authority or access to protected Household state.
_Avoid_: User subscription, Stripe subscription

**Bring-your-own Intelligence**:
Cloud assistance configured entirely through Luna's interface using a supported provider connection and billed to that connection's owner. It is available to free and paid Households.
_Avoid_: Connect ChatGPT, connect Claude

**Local-only Intelligence**:
The policy that restricts Luna to deterministic local processing and any approved on-device model.
