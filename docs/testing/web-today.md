# Today frontend validation

**Status:** Web-to-engine validation guidance

The production frontend is available at `http://localhost:3001/today` during local review. Its default `TodayService` uses the Rust backend and direct server-side OpenAI adapter. The in-browser mock remains injectable for isolated route tests. Gmail, cloud storage, authentication, workers and Daily Briefing generation are not connected.

Start the backend with `OPENAI_API_KEY` and an explicit `LUNA_OPENAI_MODEL`, then start the web application. Never use a `NEXT_PUBLIC_` variable for the key. The local backend and its `.luna/web` data directory are private review infrastructure.

## Automated evidence

- Web tests cover the historical prototype, production `Today` interactions, mock-adapter contracts and the HTTP `TodayService` request boundary.
- The 33 production-route tests cover briefing, navigation, view selection, optional conversation-context removal, one global conversation, explicit and ambiguous reference handling, household-wide questions, approval, retry, correction, dismissal, completion, bounded attachment selection, button and keyboard submission, optimistic conversation updates, duplicate-send prevention, focus restoration, attachment-plus-message handling, draft preservation, loading, empty, unavailable, recoverable-error, partial-failure and mobile drawer behavior.
- Eight mock-adapter contract tests cover isolated views, global chronological messages, explicit reference resolution, selected-context hints, ambiguity without mutation, multiple affected work items, unrelated-state preservation and bounded sources.
- Rust API tests cover bounded upload, safe source identifiers, transactional rollback, server-derived identity, the deterministic uploaded-document founder journey, refresh persistence and Luna-owned approval.
- The deterministic end-to-end backend scenario covers image upload, creation, a read-only account question, partial fact correction without duplication, explicit completion, removal from attention and durable history after restart.
- TypeScript type checking passes.
- The Next.js production build passes and emits `/today` as a static route.
- No lint script or standalone accessibility runner is currently configured in `apps/web`.
- `git diff --check` passes.

## Browser acceptance

The real `/today` route was reviewed in the in-app Chromium browser at:

| Viewport | Result | Evidence |
| --- | --- | --- |
| 1440 × 1000 | Passed | Three-part layout, one H1, persistent composer, selected working context, no horizontal overflow |
| 900 × 1000 | Passed | Compact named navigation, primary stream retained, working context opens as a keyboard-focused drawer, no horizontal overflow |
| 390 × 844 | Passed | One-column stream, compact navigation, composer above navigation, context details available, no horizontal overflow, visible controls at least 40px high |

The browser review also confirmed:

- no Variant B/C switcher or Dashboard entry on `/today`;
- no unlabeled visible buttons or form controls;
- Escape closes contextual navigation and work drawers, with focus restored to the opener;
- status changes use live regions;
- no internal provider, contract, confidence or `Obligation` language is visible;
- mocked approval updates the same work without leaving an approval control behind; and
- no browser console errors or warnings occurred.

Repeat this browser acceptance and the opt-in uploaded-document OpenAI journey on the final PR head before merge. The live test must report only sanitised provider metadata; it must not retain source content, raw output or credentials.
