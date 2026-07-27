# Luna First Vertical — Implementation Tickets

This review document decomposes [specification issue #1](https://github.com/yasserchehade/Luna/issues/1) into small, dependency-aware tracer-bullet tickets.

- Status: approved and published
- Tracker: private GitHub repository
- Triage label: `ready-for-agent`
- Development strategy: complete the unfamiliar-document-to-learned-automation path depth-first, implementing only dependencies that genuinely block it

## Dependency chart

An arrow means **dependee/blocker → dependent ticket**.

```mermaid
flowchart TD
    T01["01 · #2"] --> T02["02 · #3"]
    T02 --> T03["03 · #4"]
    T03 --> T04["04 · #5"]
    T04 --> T05["05 · #6"]
    T05 --> T06["06 · #7"]
    T06 --> T07["07 · #8"]
    T07 --> T08["08 · #9"]
    T08 --> T09["09 · #10"]

    T09 --> T10["10 · #11"]
    T09 --> T11["11 · #12"]
    T09 --> T12["12 · #13"]
    T08 --> T13["13 · #14"]

    T03 --> T14["14 · #15"]
    T10 --> T14
    T11 --> T14
    T12 --> T14
    T13 --> T14

    T11 --> T15["15 · #16"]
    T12 --> T15
    T13 --> T15
    T14 --> T15
```

The main value spine is tickets 01–09. Tickets 10–13 deepen transparency, duplicate handling, cloud reasoning and resilience. Ticket 14 reunites those branches through portable encrypted memory. Ticket 15 is final cross-platform verification.

## Ticket index

| Ticket | GitHub | Title | Blocked by |
|---|---:|---|---|
| 01 | [#2](https://github.com/yasserchehade/Luna/issues/2) | Bootstrap the clean-sheet Luna desktop | None |
| 02 | [#3](https://github.com/yasserchehade/Luna/issues/3) | Create the required Luna account and household | 01 |
| 03 | [#4](https://github.com/yasserchehade/Luna/issues/4) | Enrol and recover a trusted device | 02 |
| 04 | [#5](https://github.com/yasserchehade/Luna/issues/5) | Give Luna a desk and create the cabinet | 03 |
| 05 | [#6](https://github.com/yasserchehade/Luna/issues/6) | Attach a document to a durable conversation | 04 |
| 06 | [#7](https://github.com/yasserchehade/Luna/issues/7) | Inspect a document locally | 05 |
| 07 | [#8](https://github.com/yasserchehade/Luna/issues/8) | Clarify household context and choose a destination | 06 |
| 08 | [#9](https://github.com/yasserchehade/Luna/issues/9) | File and verify an untouched original | 07 |
| 09 | [#10](https://github.com/yasserchehade/Luna/issues/10) | Learn a filing rule and automate the next match | 08 |
| 10 | [#11](https://github.com/yasserchehade/Luna/issues/11) | Expose learned behaviour and owner corrections | 09 |
| 11 | [#12](https://github.com/yasserchehade/Luna/issues/12) | Resolve duplicates and document versions | 09 |
| 12 | [#13](https://github.com/yasserchehade/Luna/issues/13) | Add consent-gated cloud intelligence | 09 |
| 13 | [#14](https://github.com/yasserchehade/Luna/issues/14) | Survive offline and unavailable-cabinet conditions | 08 |
| 14 | [#15](https://github.com/yasserchehade/Luna/issues/15) | Synchronise encrypted portable Luna memory | 03, 10, 11, 12, 13 |
| 15 | [#16](https://github.com/yasserchehade/Luna/issues/16) | Verify the complete cross-platform golden path | 11, 12, 13, 14 |

## 01 — Bootstrap the clean-sheet Luna desktop

**GitHub:** [#2](https://github.com/yasserchehade/Luna/issues/2)
**Blocked by:** None — can start immediately.

**What it delivers:** A launchable Tauri, React and Rust application with Windows/macOS builds, the five primary destinations, a local database and the shared application-level acceptance-test seam.

### Acceptance criteria

- [ ] The application launches in development on Windows and macOS.
- [ ] The shell exposes Luna, To do, Cabinet, History and Options without reusing the previous workbench design.
- [ ] A per-device local database persists and reloads a small application setting.
- [ ] Cross-platform CI builds the desktop shell on Windows and macOS.
- [ ] An application-level seam exists for submitting work and observing user-visible state.

## 02 — Create the required Luna account and household

**GitHub:** [#3](https://github.com/yasserchehade/Luna/issues/3)
**Blocked by:** 01 / #2.

**What it delivers:** A first owner can register, sign in and create the household that employs Luna, establishing durable identity before sensitive cabinet work begins.

### Acceptance criteria

- [ ] A new owner can register and verify a Luna account.
- [ ] The owner can sign in again and return to the same household.
- [ ] The household records one owner while remaining compatible with future private and shared adult spaces.
- [ ] Authentication failures do not reveal whether another person’s account exists.
- [ ] The account-and-household journey is covered through the application seam.

## 03 — Enrol and recover a trusted device

**GitHub:** [#4](https://github.com/yasserchehade/Luna/issues/4)
**Blocked by:** 02 / #3.

**What it delivers:** The owner’s first desktop becomes a trusted Luna device with protected keys, authenticator MFA and recovery that remains separate from account-password recovery.

### Acceptance criteria

- [ ] The owner can configure authenticator-based MFA.
- [ ] The device creates its own key pair and protects private material in the operating-system credential vault.
- [ ] Onboarding generates an offline household recovery key and requires confirmation.
- [ ] Resetting the Luna account password does not decrypt household memory.
- [ ] A recovery-key flow can enrol a replacement trusted device.
- [ ] A revoked or incorrectly keyed device cannot read encrypted household state.

## 04 — Give Luna a desk and create the cabinet

**GitHub:** [#5](https://github.com/yasserchehade/Luna/issues/5)
**Blocked by:** 03 / #4.

**What it delivers:** The owner chooses Luna’s local or cloud-synchronised desk, reviews a suggested household cabinet and creates an ordinary folder structure they continue to own.

### Acceptance criteria

- [ ] The owner can select a writable filesystem folder using a native picker.
- [ ] Luna previews a recommended cabinet preset before changing the filesystem.
- [ ] The owner can rename, add or remove suggested cabinet sections.
- [ ] No folders are created until the owner confirms the preview.
- [ ] Created folders are human-readable and usable outside Luna.
- [ ] Luna remembers and validates the cabinet on the next launch.

## 05 — Attach a document to a durable conversation

**GitHub:** [#6](https://github.com/yasserchehade/Luna/issues/6)
**Blocked by:** 04 / #5.

**What it delivers:** The owner starts a familiar Luna conversation, attaches a supported document and can trust that the arrival persists independently while unresolved action appears in To do.

### Acceptance criteria

- [ ] The owner can create, rename, search, archive and delete conversations.
- [ ] A PDF, JPG or PNG can be attached through selection or drag-and-drop.
- [ ] The document arrival remains durable if its conversation is deleted.
- [ ] Processing state is visible in the originating conversation.
- [ ] Required owner action appears once in To do and opens the exact item.
- [ ] Resolving work in either surface updates the same durable state.

## 06 — Inspect a document locally

**GitHub:** [#7](https://github.com/yasserchehade/Luna/issues/7)
**Blocked by:** 05 / #6.

**What it delivers:** Luna performs a privacy-preserving first read that validates and fingerprints a document, extracts local text or OCR, preserves the original and presents understandable evidence.

### Acceptance criteria

- [ ] Luna validates actual file type and safely rejects unsupported or malformed input.
- [ ] The exact original bytes and original filename are preserved.
- [ ] Luna calculates and records a cryptographic checksum.
- [ ] Text is extracted locally from digital PDFs.
- [ ] JPG, PNG and image-only PDF content can be OCRed locally without modifying the original.
- [ ] The conversation shows a structured review card with evidence and plain-language confidence.
- [ ] No document content leaves the device.

## 07 — Clarify household context and choose a destination

**GitHub:** [#8](https://github.com/yasserchehade/Luna/issues/8)
**Blocked by:** 06 / #7.

**What it delivers:** Luna asks focused questions about an unfamiliar document and turns the owner’s answers into confirmed context and a filing decision.

### Acceptance criteria

- [ ] The review card represents document type, provider, addressee, property, account, amount, dates and destination.
- [ ] Luna asks only unresolved questions that can change identity, context or filing.
- [ ] The owner can correct any extracted field.
- [ ] A new provider or address remains unresolved until the owner explains its relevance.
- [ ] Luna proposes a readable filename and cabinet path from confirmed context.
- [ ] The owner can edit and confirm the destination.
- [ ] An unfamiliar document cannot proceed as if its context were confirmed.

## 08 — File and verify an untouched original

**GitHub:** [#9](https://github.com/yasserchehade/Luna/issues/9)
**Blocked by:** 07 / #8.

**What it delivers:** Luna safely stages, files and verifies the untouched original while Conversation, To do, Cabinet and History report one consistent outcome.

### Acceptance criteria

- [ ] Luna stages the original before a cabinet write.
- [ ] Generated paths stay inside the cabinet and are valid on Windows and macOS.
- [ ] Existing files are never overwritten silently.
- [ ] The filed copy matches the staged checksum.
- [ ] Luna records original name, final path, checksum, source and filing decision.
- [ ] Staging is removed only after verification and durable event recording.
- [ ] Conversation, To do, Cabinet and History agree that filing completed.

## 09 — Learn a filing rule and automate the next match

**GitHub:** [#10](https://github.com/yasserchehade/Luna/issues/10)
**Blocked by:** 08 / #9.

**What it delivers:** One confirmed filing becomes earned autonomy: Luna shows the learned scope and automatically handles the next genuinely matching document while escalating changed context.

### Acceptance criteria

- [ ] Luna presents the learned rule in plain language after filing.
- [ ] A rule can match document type, provider, addressee and property or account.
- [ ] A second exact contextual match files automatically.
- [ ] Changed provider, addressee, property, account or document type does not inherit the rule silently.
- [ ] Automatic filing preserves transactional verification and audit behaviour.
- [ ] The accepted golden path passes through the application seam.

## 10 — Expose learned behaviour and owner corrections

**GitHub:** [#11](https://github.com/yasserchehade/Luna/issues/11)
**Blocked by:** 09 / #10.

**What it delivers:** The owner gets a transparent rulebook, while direct cabinet changes become authoritative teaching moments rather than actions Luna reverses.

### Acceptance criteria

- [ ] Options lists every rule, its scope, teacher, creation time and affected documents.
- [ ] The owner can pause, edit and delete a rule.
- [ ] Rule edits apply prospectively by default.
- [ ] Historical reorganisation shows an exact preview and requires approval.
- [ ] A manual document move asks whether it teaches a rule or is a one-off.
- [ ] Luna never silently reverses an owner’s move.
- [ ] Rule and correction decisions appear in History.

## 11 — Resolve duplicates and document versions

**GitHub:** [#12](https://github.com/yasserchehade/Luna/issues/12)
**Blocked by:** 09 / #10.

**What it delivers:** The owner controls the first duplicate decision and Luna can learn a narrow exact-duplicate preference without discarding revisions.

### Acceptance criteria

- [ ] Exact byte duplicates and likely semantic duplicates are distinguished.
- [ ] The first duplicate offers keep both, link copies, discard or updated version.
- [ ] The owner can create a scoped future preference for exact duplicates.
- [ ] Exact-duplicate preference does not apply to similar files with changed bytes.
- [ ] Updated versions retain both originals and an explicit relationship.
- [ ] Duplicate handling never overwrites an existing document.
- [ ] Decisions and provenance appear in Conversation and History.

## 12 — Add consent-gated cloud intelligence

**GitHub:** [#13](https://github.com/yasserchehade/Luna/issues/13)
**Blocked by:** 09 / #10.

**What it delivers:** Difficult documents can use one evaluated Luna-managed cloud model without surrendering provider neutrality, local authority or explicit privacy boundaries.

### Acceptance criteria

- [ ] A Luna-owned provider-neutral Intelligence Gateway returns validated Evidence and typed candidate Direction Interpretations without action authority.
- [ ] OpenAI `gpt-4.1-mini` passes the document evaluation contract through the provisional remote LiteLLM adapter, and the deterministic test gateway passes the same contract without a paid call.
- [ ] Luna selects and names the exact provider/model and explains the bounded disclosure before offering Allow once, scoped future consent or Keep local.
- [ ] One-time and reusable Consent Grants bind provider, model, capability, member and disclosed scope; they are inspectable and revocable.
- [ ] Luna-managed upstream provider credentials remain server-side. The desktop's narrow gateway credential remains in the operating-system vault and never enters SQLite, frontend storage, Cabinet content or History.
- [ ] Provider, model, schema and owning-domain validation treat every external result as untrusted and prevent direct durable-state mutation.
- [ ] Safe retry uses only the unchanged provider/model; failure leaves Document Handling waiting and never silently switches providers.
- [ ] Existing Filing Rules, duplicate handling and Document Version preservation continue without gateway availability.
- [ ] History records provider, model, reason, Consent Grant, outcome and candidate disposition without secrets or document content.
- [ ] LiteLLM remains private infrastructure replaceable by Portkey, a direct adapter or another gateway without changing Document Handling.

## 13 — Survive offline and unavailable-cabinet conditions

**GitHub:** [#14](https://github.com/yasserchehade/Luna/issues/14)
**Blocked by:** 08 / #9.

**What it delivers:** Luna continues known local work and recovers safely from connectivity or cabinet failure without guessing, losing originals or redirecting files.

### Acceptance criteria

- [ ] Known learned-rule filing completes offline when the cabinet is available.
- [ ] An unfamiliar document needing deeper intelligence waits explicitly while offline.
- [ ] An unavailable cabinet leaves the original in encrypted staging.
- [ ] Luna never chooses a different cabinet because the configured one is unavailable.
- [ ] Retry removes staging only after checksum verification.
- [ ] Persistent unavailability or time risk creates one clear To do item.
- [ ] Interrupted filing resumes without duplicate files or missing history.

## 14 — Synchronise encrypted portable Luna memory

**GitHub:** [#15](https://github.com/yasserchehade/Luna/issues/15)
**Blocked by:** 03 / #4, 10 / #11, 11 / #12, 12 / #13 and 13 / #14.

**What it delivers:** Learned behaviour and history become portable across trusted devices through signed encrypted records beside the cabinet, while every device retains a separate local database.

### Acceptance criteria

- [ ] The cabinet contains a reserved encrypted portable-memory area.
- [ ] Portable records are encrypted, signed and append-only.
- [ ] API keys, tokens, device private keys and plaintext secrets never enter portable memory.
- [ ] A new trusted device rebuilds rules, relationships and History into its local database.
- [ ] Duplicate event delivery is idempotent and modified or invalid replayed records are rejected.
- [ ] Concurrent events produce a detectable, resolvable conflict rather than silent overwrite.
- [ ] No live database is synchronised through the cabinet.

## 15 — Verify the complete cross-platform golden path

**GitHub:** [#16](https://github.com/yasserchehade/Luna/issues/16)
**Blocked by:** 11 / #12, 12 / #13, 13 / #14 and 14 / #15.

**What it delivers:** The first clean-sheet competency is proven dependable on Windows and macOS before adjacent beta features expand.

### Acceptance criteria

- [ ] The unfamiliar-document-to-learned-automation path passes repeatedly on Windows and macOS.
- [ ] Changed provider, property and addressee variants return to clarification.
- [ ] Duplicate resolution, denied cloud permission, provider outage, offline work and cabinet recovery pass through the visible seam.
- [ ] Interrupted filing never loses or silently overwrites an original.
- [ ] No provider fallback occurs without consent.
- [ ] No credential or plaintext key appears in cabinet records, portable events, logs or crash output.
- [ ] Keyboard and accessibility checks pass for attachment, review cards, To do and cabinet selection.
- [ ] Signed installable beta artifacts and smoke-test evidence exist for both operating systems.

## Execution guidance

Start with the dependency frontier. Initially only ticket 01 / issue #2 is unblocked. Complete and verify one ticket before moving to the next available frontier item. Do not close or repurpose the parent specification issue.
