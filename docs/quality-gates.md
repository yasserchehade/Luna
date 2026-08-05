# Luna quality gates

This plan defines stop points that prevent Luna from advancing while behavior, safety or specification compliance is unproven. ADR 0020 makes web validation primary and retains desktop gates only for explicitly approved deferred work.

## Approved test seams

Tests observe behavior through public boundaries:

1. **Web application seam** — drive the responsive browser interface like a household member and observe the Today briefing, Conversation, Household Work, working context and visible outcomes.
2. **Web service seam** — invoke versioned household API commands with authenticated household context and real test persistence once the service exists.
3. **Boundary contracts** — replace only external boundaries such as intelligence, identity, storage-provider adapters, clocks, network availability and execution tools.
4. **Deferred desktop seams** — local Rust commands, installed Tauri behavior and packaged Windows/macOS artifacts remain valid only for approved desktop maintenance or extraction work.

Tests must not reach into private functions, inspect persistence as a shortcut around a public interface or treat model output as accepted state. Expected results come from specifications, fixtures and worked examples rather than restating implementation.

The production `/today` frontend uses interaction tests through its rendered route plus contract tests through the `TodayService` seam. The current adapter is fixture-backed; browser acceptance runs at `/today` across desktop, tablet and mobile widths. A future service integration must add API-boundary coverage without replacing these household-member-facing checks or making snapshots the primary evidence.

Current frontend evidence and exact review instructions are recorded in [Today frontend validation](./testing/web-today.md).

Historical Tauri references remain available for deferred desktop work:

- <https://v2.tauri.app/develop/tests/>
- <https://v2.tauri.app/develop/tests/webdriver/>
- <https://v2.tauri.app/distribute/pipelines/github/>

## Universal stop points

### Every implementation slice

- Add one behavioral test at an approved seam.
- Observe the expected failure before relying on the test as regression evidence where practical.
- Add only enough implementation for the behavior.
- Keep mock/prototype state visibly separate from production claims.
- Defer unrelated refactoring and integrations.

### Every local commit

- Formatting and linting pass for changed surfaces.
- Type checking passes.
- Focused and complete relevant tests pass.
- Relevant production or review build passes.
- Secret and sensitive-output checks pass.
- `git diff --check` passes.
- The diff contains no unrelated changes.

### Every ticket pull request

- Every acceptance criterion is demonstrated through an approved seam.
- Web changes pass web tests, typecheck and production build.
- Extracted Rust/service changes pass Rust formatting, complete tests and strict Clippy.
- Windows/macOS desktop checks run only when deferred desktop code, packaging or a shared crate they consume changes.
- Account-service lint and contract checks run when account or Supabase paths change.
- Production web behavior has a final-head browser acceptance result before merge.
- Standards and Spec reviews pass against repository guidance and the originating issue.
- No unresolved P0 or P1 finding remains.
- CI and required manual evidence are linked from the issue or PR.

## GitHub Actions execution policy

CI must be path-scoped by product surface:

- `apps/web/**` changes run web tests, typecheck and build;
- Rust or desktop paths retain Ubuntu core and Windows/macOS checks where relevant;
- account-service workflows remain scoped to account or Supabase paths; and
- documentation-only changes do not start runtime workflows.

Path-filtered workflow names must not be configured as unconditional repository ruleset checks because skipped workflows may remain pending. Use always-created aggregate checks or rules scoped to protected paths.

All workflows use standard GitHub-hosted runners unless a separate operational decision approves otherwise. Dependencies should use supported package and Rust caching, and same-ref concurrency should cancel superseded runs.

## Current checkpoints

| Checkpoint | Required evidence before continuing |
| --- | --- |
| Web experience selection | Variant A is selected and promoted to `/today`; final production-route review must confirm navigation, persistent composer, context, work actions and loading/empty/failure states across desktop, tablet and mobile. |
| Web API contract | Authenticated Household Work, Conversation, briefing projection and bounded upload commands have explicit authority, concurrency, idempotency and audit behavior. |
| Uploaded-document web slice | One bounded source becomes one durable Household Work item; read-only, clarification, correction, approval, completion and dismissal pass through the web seam without duplication. |
| Connector readiness | The uploaded-document journey is stable before email or user-controlled storage adapters begin. |

The historical desktop foundation, Cabinet, filing, portable-memory and installed-app checkpoints remain evidence for deferred capability; they do not block the web prototype or become web MVP prerequisites.

## Managed intelligence environment gates

OpenAI and gateway credentials remain server-side and never enter browser code, source content, Household Work or diagnostics. Any live-provider canary uses synthetic content, disposable credentials, exact route/schema checks and body-free logs.

Before managed intelligence is available to external web testers, its service boundary must use authenticated TLS ingress, managed secrets, attributable access, abuse controls and verified content-free operational logs. The browser receives only authenticated Luna API behavior, never a provider or gateway credential.

Provider choice, BYOK and customer provider-key UI remain deferred and must not appear in the primary web experience.

## Severity and release policy

| Severity | Meaning | Policy |
| --- | --- | --- |
| P0 | Data loss, privacy breach, authority violation or secret exposure | Stop immediately; blocks commit, merge and release. |
| P1 | Acceptance criterion or core workflow failure | Blocks merge and dependent work. |
| P2 | Non-critical usability or presentation defect | May be deferred only through an explicit recorded decision. |

Failures and waivers must remain visible. A flaky test is failing until its cause is understood.

## Evidence record

Before an implementation issue closes, attach or link:

- CI and test results;
- acceptance-criterion demonstrations;
- responsive, accessibility and browser-specific evidence where automation is insufficient;
- Standards and Spec review outcomes; and
- accepted P2 deferrals with follow-up ownership.
