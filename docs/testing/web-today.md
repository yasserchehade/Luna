# Today frontend validation

**Status:** Local founder-review evidence for `codex/promote-variant-a-today`

The production frontend foundation is available at `http://localhost:3001/today` during local review. It uses the in-browser mock `TodayService`; it does not connect authentication, persistence, OpenAI, Gmail, storage, workers or Daily Briefing generation.

## Automated evidence

- 56 web tests pass across the historical prototype, production `Today` interactions and mock-adapter contracts.
- The 33 production-route tests cover briefing, navigation, view selection, optional conversation-context removal, one global conversation, explicit and ambiguous reference handling, household-wide questions, approval, retry, correction, dismissal, completion, bounded attachment selection, button and keyboard submission, optimistic conversation updates, duplicate-send prevention, focus restoration, attachment-plus-message handling, draft preservation, loading, empty, unavailable, recoverable-error, partial-failure and mobile drawer behavior.
- Eight mock-adapter contract tests cover isolated views, global chronological messages, explicit reference resolution, selected-context hints, ambiguity without mutation, multiple affected work items, unrelated-state preservation and bounded sources.
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

Repeat this browser acceptance on the final PR head before merge. The draft PR and founder review do not constitute backend or automation acceptance.
