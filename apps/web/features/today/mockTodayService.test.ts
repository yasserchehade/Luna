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

  it("keeps message replies on the selected household work", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const result = await service.sendMessage({ workId: "electricity-bill", message: "Please keep this open until I pay it." });
    expect(result.briefing.work).toHaveLength(4);
    expect(result.work?.conversation.map((entry) => entry.message)).toContain("Please keep this open until I pay it.");
  });

  it("accepts bounded MVP sources and rejects unsupported or oversized files", async () => {
    const service = createMockTodayService({ latencyMs: 0 });
    const accepted = await service.attachSource(new File(["fixture"], "bill.pdf", { type: "application/pdf" }));
    expect(accepted.displayName).toBe("bill.pdf");

    await expect(service.attachSource(new File(["fixture"], "bill.txt", { type: "text/plain" }))).rejects.toMatchObject({ code: "invalidAttachment" });
    await expect(service.attachSource(new File([new Uint8Array(5 * 1024 * 1024 + 1)], "scan.png", { type: "image/png" }))).rejects.toMatchObject({ code: "invalidAttachment" });
  });
});
