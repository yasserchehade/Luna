import { describe, expect, it } from "vitest";
import { createInitialState, layoutModeForWidth, prototypeReducer, visibleAttention } from "./prototypeState";

describe("web-first prototype state", () => {
  it("navigates without losing the active Household Work", () => {
    const state = createInitialState();
    const next = prototypeReducer(state, { type: "navigate", destination: "Calendar" });
    expect(next.activeNavigation).toBe("Calendar");
    expect(next.selectedWorkId).toBe(state.selectedWorkId);
  });

  it("supports approval, dismissal and completion as local mock transitions", () => {
    let state = createInitialState();
    state = prototypeReducer(state, { type: "approve", workId: "electricity-bill" });
    expect(state.works.find((work) => work.id === "electricity-bill")?.status).toBe("upcoming");
    state = prototypeReducer(state, { type: "dismiss", workId: "insurance-renewal" });
    expect(visibleAttention(state.works).map((work) => work.id)).not.toContain("insurance-renewal");
    state = prototypeReducer(state, { type: "complete", workId: "school-form" });
    expect(state.works.find((work) => work.id === "school-form")?.status).toBe("completed");
  });

  it("applies a conversational correction without replacing unrelated facts", () => {
    const state = createInitialState();
    const next = prototypeReducer(state, { type: "saveCorrection", workId: "electricity-bill", value: "This bill is for the rental property." });
    const work = next.works.find((candidate) => candidate.id === "electricity-bill");
    expect(work?.summary).toBe("This bill is for the rental property.");
    expect(work?.facts.find((fact) => fact.label === "Amount")?.value).toBe("$184.72");
  });

  it("closes a cancelled correction without changing Household Work", () => {
    const state = prototypeReducer(createInitialState(), { type: "openCorrection", workId: "electricity-bill" });
    const next = prototypeReducer(state, { type: "cancelCorrection" });
    expect(next.correctionOpen).toBe(false);
    expect(next.works).toEqual(state.works);
  });

  it("records attachment selection and clears it after a mock send", () => {
    let state = prototypeReducer(createInitialState(), { type: "attach", filename: "renewal.pdf" });
    expect(state.attachmentName).toBe("renewal.pdf");
    state = prototypeReducer(state, { type: "send" });
    expect(state.attachmentName).toBeNull();
  });

  it("classifies mobile, tablet and desktop widths", () => {
    expect(layoutModeForWidth(390)).toBe("mobile");
    expect(layoutModeForWidth(900)).toBe("tablet");
    expect(layoutModeForWidth(1440)).toBe("desktop");
  });
});
