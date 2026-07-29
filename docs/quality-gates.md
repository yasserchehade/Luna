# Luna quality gates

This plan defines the stop points that prevent Luna from advancing while behavior, safety or specification compliance is unproven. These gates are additive to each implementation ticket's acceptance criteria.

## Approved test seams

Tests observe behavior only through these public boundaries:

1. **Application seam** — drive the installed Tauri interface like a household member and observe Conversation, To do, Cabinet, History and Options.
2. **Local-core seam** — invoke Luna's public Rust commands using a real temporary local database and temporary cabinet.
3. **Boundary contracts** — replace only external boundaries such as intelligence providers, credential vaults, clocks, network availability and native dialogs.
4. **Packaged-app seam** — install and smoke-test the actual Windows and macOS artifacts.

Tests must not reach into private functions, mock Luna's internal collaborators or inspect the database as a shortcut around a public interface. Expected results must come from specifications, fixtures or worked examples rather than restating the implementation.

Member-direction interpretation is a boundary contract: deterministic test interpreters may propose typed candidate directions, but tests must observe acceptance, clarification, refusal and execution through the local-core or installed-application seam. Interpretation test doubles do not bypass owning-domain validation.

Tauri supports unit and integration testing through its mock runtime and desktop testing on Windows and macOS through WebdriverIO's Tauri service:

- <https://v2.tauri.app/develop/tests/>
- <https://v2.tauri.app/develop/tests/webdriver/>
- <https://v2.tauri.app/distribute/pipelines/github/>

## Universal stop points

### Every TDD slice

- Write one behavioral test at an approved seam.
- Observe it fail for the expected reason.
- Add only enough implementation to make it pass.
- Do not start another behavior while the slice is red.
- Defer refactoring until review.

### Every local commit

- Formatting passes.
- Linting passes.
- Type checking passes.
- The focused test passes.
- The complete local suite passes.
- Secret and sensitive-output checks pass.
- The diff contains no unrelated changes.

### Every ticket pull request

- Every acceptance criterion is demonstrated through an approved seam.
- The required Ubuntu core check is green: Rust formatting, the complete Rust suite, strict Clippy and TypeScript typechecking.
- Windows and macOS desktop build checks are green.
- Account-service lint and contract checks are green when account or Supabase paths change.
- A final-head `Full desktop validation` run is green before merging changes that affect runtime behaviour, the interface, configuration or packaging.
- The Standards review passes against repository guidance and the smell baseline.
- The Spec review passes against the originating GitHub issue and parent specification.
- No unresolved P0 or P1 finding remains.
- CI output and any required manual evidence are attached to the issue.

## GitHub Actions execution policy

Pull requests run the fast core suite once on `ubuntu-latest` and compile the desktop application on `windows-latest` and `macos-latest`. The account-service workflow is path-scoped so unrelated desktop changes do not start Supabase. Documentation-only changes do not start desktop workflows.

Full installed-application E2E and release-mode no-bundle builds run automatically after relevant changes reach `main`. They can also be dispatched manually against the final pull-request head. This avoids repeating the most expensive Windows and macOS work after every intermediate push without weakening the final acceptance gate: runtime, interface, configuration and packaging changes still require a successful manually dispatched full run before merge.

All workflows use only standard GitHub-hosted runners. Dependency downloads use pnpm caching through `actions/setup-node`, while Rust registry, Git and suitable `target` outputs use `Swatinem/rust-cache`. Workflow-level concurrency cancels superseded runs on the same ref.

Path-filtered workflow names must not be configured as unconditional repository ruleset checks because GitHub leaves a path-skipped required workflow pending. If required status checks are added, use always-created aggregate checks or rules scoped to the paths they protect.

## Milestone checkpoints

| Checkpoint | Closing issue | Required evidence before continuing |
| --- | --- | --- |
| Foundation | #2 | Tauri launches on Windows and macOS, the approved Option A shell renders, a local setting survives restart and the application test seam works. |
| Trust and cabinet | #5 | Account, MFA, trusted-device recovery, cabinet preview, filesystem ownership and access-denial cases pass; a manual recovery drill is recorded. |
| First filing | #9 | PDF, JPG and PNG fixtures pass from arrival through clarification and verified filing; the original remains untouched; interrupted-write recovery is proven. |
| Learned behavior | #12 | A second contextual match is handled automatically; changed provider, property, addressee, account or document type returns to clarification; corrections and duplicates remain reversible. |
| Resilience | #15 | Consent denial, provider failure, offline work, unavailable cabinet, replay, tampering and concurrent-device conflicts are exercised without silent fallback, overwrite or secret exposure. |
| Beta release | #16 | The repeated golden path passes using signed Windows and macOS installers; keyboard, accessibility, recovery, data-loss and security checks have recorded evidence. |

A milestone-closing issue cannot close while its checkpoint is incomplete, even if its own narrower acceptance criteria pass.

## Cloud Assistance environment gates

The prototype's evaluated-real-provider criterion may be demonstrated by the pinned LiteLLM deployment running ephemerally on an operator's loopback interface. It must use the fixed synthetic canary, disposable credentials and the same privacy, exact-route, structured-result, usage and revocation checks as a remote deployment. This is release-environment evidence, not a standard-suite dependency or a supported desktop sidecar.

Before Luna-managed Intelligence is available to external testers, the gateway must be remotely operated behind authenticated TLS ingress with managed secrets, attributable client credentials, abuse controls and verified body-free logs. ADR 0018 selects a named Cloudflare Tunnel from the operator-controlled prototype machine as the no-separate-host-cost internal-beta ingress, with distinct customer and Access-protected administration hostnames. Issue #53 owns the running deployment and evidence gate; it does not block completion of issue #13's prototype contract after the local real-provider canary passes.

Managed gateway credentials are provisioned automatically and are not customer configuration. Bring-your-own Intelligence is enabled only through Options after the issue #55 canary proved BYOK-only process and virtual-key isolation, provider-key non-persistence, credential-free logs, missing-key failure and HTTP 403 for a managed route. Remote use still depends on issue #53's authenticated TLS ingress evidence.

## Severity and release policy

| Severity | Meaning | Policy |
| --- | --- | --- |
| P0 | Data loss, privacy breach, authority violation or secret exposure | Stop immediately; blocks commit, merge and release. |
| P1 | Acceptance criterion or core workflow failure | Blocks merge and progression to dependent work. |
| P2 | Non-critical usability or presentation defect | May be deferred only by an explicit decision recorded on the issue. |

Failures and waivers must be visible. A check cannot be ignored silently, and a flaky test is treated as a failing test until its cause is understood.

## Evidence record

Each implementation issue is the record for its quality gate. Before closing it, attach or link:

- CI runs and test results;
- acceptance-criterion demonstrations;
- platform-specific manual results where automation is insufficient;
- Standards and Spec review outcomes;
- any accepted P2 deferral, including its follow-up issue.
