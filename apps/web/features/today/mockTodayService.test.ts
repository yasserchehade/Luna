import { describe, expect, it } from "vitest";
import { createMockTodayService } from "./mockTodayService";

describe("mock Today service", () => {
  it("returns isolated web-facing briefing views", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const first = await service.getBriefing();
    first.work[0].title = "Changed outside the adapter";
    const second = await service.getBriefing();
    expect(second.work[0].title).toBe("Electricity bill needs approval");
  });

  it("corrects one fact without duplicating work or replacing unrelated facts", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const before = await service.getWorkItem("insurance-renewal");
    const result = await service.correctFact({ workId: before.id, factKey: "excess", value: "$750" });
    const corrected = result.work;

    expect(result.briefing.work).toHaveLength(4);
    expect(corrected?.facts.find((fact) => fact.key === "excess")?.value).toBe("$750");
    expect(corrected?.facts.find((fact) => fact.key === "premium")?.value).toBe("$1,248 yearly");
    expect(corrected?.source).toEqual(before.source);
  });

  it("records messages in one global conversation instead of on household work", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const result = await service.sendMessage({ contextualWorkIds: ["electricity-bill"], message: "Please keep this open until I pay it." });
    expect(result.briefing.work).toHaveLength(4);
    expect(result.briefing.conversation.map((entry) => entry.body)).toContain("Please keep this open until I pay it.");
    expect(result.briefing.work.every((work) => !("conversation" in work))).toBe(true);
  });

  it("resolves an explicit electricity reference without selected context", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const result = await service.sendMessage({ message: "I already paid the electricity bill." });

    expect(result.affectedWorkIds).toEqual(["electricity-bill"]);
    expect(result.briefing.work.find((work) => work.id === "electricity-bill")?.status).toBe("completed");
    expect(result.briefing.work.find((work) => work.id === "school-form")?.status).toBe("upcoming");
  });

  it("asks for clarification and changes no work when a pronoun is ambiguous", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const before = await service.getBriefing();
    const result = await service.sendMessage({ message: "I already paid it." });

    expect(result.clarification?.question).toBe("Which item did you pay?");
    expect(result.affectedWorkIds).toEqual([]);
    expect(result.briefing.work.map(({ id, status }) => ({ id, status }))).toEqual(
      before.work.map(({ id, status }) => ({ id, status })),
    );
  });

  it("uses selected context as a hint without constraining a global attention question", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const hinted = await service.sendMessage({ contextualWorkIds: ["insurance-renewal"], message: "Keep the current excess." });
    expect(hinted.affectedWorkIds).toEqual(["insurance-renewal"]);
    expect(hinted.briefing.work.find((work) => work.id === "insurance-renewal")?.needs).toBeNull();

    const global = await service.sendMessage({ contextualWorkIds: ["electricity-bill"], message: "What else needs attention?" });
    expect(global.lunaMessage.body).toContain("Rental insurance renewal");
    expect(global.lunaMessage.body).toContain("School excursion form prepared");
  });

  it("applies two explicit safe work updates in one conversational turn", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const result = await service.sendMessage({ message: "Mark the electricity bill paid and dismiss the school form." });

    expect(result.affectedWorkIds).toEqual(["electricity-bill", "school-form"]);
    expect(result.briefing.work.find((work) => work.id === "electricity-bill")?.status).toBe("completed");
    expect(result.briefing.work.find((work) => work.id === "school-form")?.status).toBe("dismissed");
    expect(result.briefing.conversation.map((entry) => entry.createdAt)).toEqual(
      [...result.briefing.conversation.map((entry) => entry.createdAt)].sort(),
    );
  });

  it("accepts bounded MVP sources and rejects unsupported or oversized files", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const accepted = await service.attachSource(new File(["fixture"], "bill.pdf", { type: "application/pdf" }));
    expect(accepted.displayName).toBe("bill.pdf");

    await expect(service.attachSource(new File(["fixture"], "bill.txt", { type: "text/plain" }))).rejects.toMatchObject({ code: "invalidAttachment" });
    await expect(service.attachSource(new File([new Uint8Array(5 * 1024 * 1024 + 1)], "scan.png", { type: "image/png" }))).rejects.toMatchObject({ code: "invalidAttachment" });
  });
});
