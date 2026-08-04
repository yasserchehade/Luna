# Luna Competency Map

**Status:** Product planning baseline

Luna grows by becoming better at household-administration competencies. Integrations provide sources and tools for those competencies; they are not the organising centre of the product.

## Competency map

| Competency | MVP | Near-term | Later | Explicitly deferred |
| --- | --- | --- | --- | --- |
| **Observe** | Accept a bounded member-uploaded PDF/JPG/PNG through the web journey; preserve a logical source reference; present fixture-backed proactive briefing states | Add one authorised incoming source after the uploaded-document loop is proven; source health and retry | Passive multi-source monitoring; household-specific source prioritisation | Gmail/Outlook and storage-provider connectors before the web work journey works; building a full email client |
| **Understand** | Read a member message and uploaded PDF/JPG/PNG with OpenAI; assemble relevant household context, conversation and prior work; identify attention, facts and urgency | Correlate multiple messages and sources; resolve entities across household history; better uncertainty explanations | Longitudinal household understanding; specialised reading competencies | Local models, local-only reasoning, multiple providers and BYOK |
| **Organise** | Create and maintain durable household work / internal `Obligation`; link sources, facts, responsibility, due date and status | Merge related updates; prioritise work; assign or suggest responsibility; handle recurring obligations | Household planning, routines and richer work categories | Generic workflow-builder primitives; document-first ownership; forms as the work model |
| **Communicate** | Use the web `Today` briefing and persistent conversation as the primary interface; explain findings; ask one focused question; propose a draft reply or reminder | Live proactive briefs; multiple household members; concise channel-aware responses | Family coordination and adaptive communication preferences | An empty generic chat, separate document chat or scripted questionnaire presented as conversation |
| **Execute** | Validate and execute a small set of safe, approved actions; create drafts and reminders; record audit outcomes | Send approved replies; calendar reminders; standing directions for low-risk repeated actions | More connected services and bounded autonomous routines | Autonomous bill payment, bank integrations and broad role/authority systems |
| **Monitor** | Keep work active until completed, dismissed or no longer relevant; track due dates, failures and waiting states | Follow-up scheduling; overdue escalation; source re-checks; duplicate prevention | Cross-source progress monitoring and household-level workload management | Extensive operational automation before durable work and recovery are proven |
| **Learn** | Record member corrections, approvals, outcomes and scoped preferences as explicit durable facts | Reuse safe decisions for similar work; explain what was learned and allow revocation | Household-specific routines and adaptive prioritisation | Advanced filing-rule learning and hidden model memory as the primary learning system |

## Source and tool boundary

The competency is the product. Sources and tools are inputs and execution surfaces:

| Source or tool | Role in Luna |
| --- | --- |
| Member upload | First web source used to prove the Household Work journey |
| Gmail or Outlook | Deferred incoming source adapter after the web journey is proven |
| PDF, JPG or PNG | Evidence attached to a source; a preserved original where required |
| Conversation | Primary interaction layer for understanding, direction, approval and explanation |
| Calendar | Future source or execution tool for dates and reminders |
| SMS and other messages | Future incoming or outgoing channels |
| OpenAI | MVP reasoning and document-reading engine |
| Luna-owned tools | Validated execution layer for drafts, reminders and authorised external actions |
| Cabinet | Logical household-document and source layer, not the product's work owner |
| User-controlled cloud storage | Future owner of file bytes, versions, sharing and storage infrastructure |

No connector should create its own separate conversation, intelligence path or household-work lifecycle. New sources should feed the shared reasoning and work model; new tools should pass through Luna's validation and approval boundary.

## Development test

For every proposed integration, ask which competency it strengthens, which household work it reduces, and whether the same member outcome could be demonstrated without making the integration visible as product configuration. If the integration cannot answer those questions, it is not MVP work.
