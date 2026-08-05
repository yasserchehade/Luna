import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { afterEach, describe, expect, it } from "vitest";
import { createHttpTodayService } from "./httpTodayService";
import type { TodayBriefing } from "./contracts";

const emptyBriefing: TodayBriefing = {
  member: { displayName: "Yasser", householdName: "Chehade household", initials: "YC" },
  dateLabel: "Wednesday, 5 August",
  greeting: "Good afternoon",
  reviewed: { emails: 0, documents: 1, calendar: false },
  conversation: [],
  work: [],
  partialFailures: [],
};

const servers: Array<ReturnType<typeof createServer>> = [];

afterEach(async () => {
  await Promise.all(servers.splice(0).map((server) => new Promise<void>((resolve) => server.close(() => resolve()))));
});

async function backend(handler: (request: IncomingMessage, response: ServerResponse, body: string) => void) {
  const server = createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => handler(request, response, body));
  });
  servers.push(server);
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("test backend did not bind");
  return `http://127.0.0.1:${address.port}/api`;
}

describe("HTTP TodayService", () => {
  it("loads the durable Today projection from the backend", async () => {
    const baseUrl = await backend((request, response) => {
      expect(request.url).toBe("/api/today");
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify(emptyBriefing));
    });

    await expect(createHttpTodayService({ baseUrl }).getBriefing()).resolves.toEqual(emptyBriefing);
  });

  it("uploads a source then sends the global conversation turn using its opaque id", async () => {
    const calls: Array<{ url: string; method: string; body: string }> = [];
    const baseUrl = await backend((request, response, body) => {
      calls.push({ url: request.url ?? "", method: request.method ?? "", body });
      response.setHeader("content-type", "application/json");
      if (request.url === "/api/sources") {
        response.statusCode = 201;
        response.end(JSON.stringify({ sourceId: "source-safe", displayName: "bill.pdf", mediaType: "application/pdf", sizeBytes: 8 }));
      } else {
        response.end(JSON.stringify({
          briefing: emptyBriefing,
          memberMessage: { id: "m1", role: "member", body: "Take care of this.", createdAt: "2026-08-05T00:00:00Z" },
          lunaMessage: { id: "m2", role: "luna", body: "I found the bill.", createdAt: "2026-08-05T00:00:01Z" },
          affectedWorkIds: ["work-1"],
        }));
      }
    });
    const service = createHttpTodayService({ baseUrl });
    const attachment = await service.attachSource(new File(["%PDF-1.4"], "bill.pdf", { type: "application/pdf" }));
    const result = await service.sendMessage({ message: "Take care of this.", attachmentId: attachment.attachmentId });

    expect(attachment.attachmentId).toBe("source-safe");
    expect(result.affectedWorkIds).toEqual(["work-1"]);
    expect(calls.map(({ url, method }) => ({ url, method }))).toEqual([
      { url: "/api/sources", method: "POST" },
      { url: "/api/conversation", method: "POST" },
    ]);
    expect(JSON.parse(calls[1].body)).toEqual({ message: "Take care of this.", contextualWorkIds: [], sourceId: "source-safe" });
  });

  it("maps an unavailable backend to the safe durable-state error", async () => {
    const baseUrl = await backend((_request, response) => {
      response.statusCode = 502;
      response.end("backend unavailable");
    });

    await expect(createHttpTodayService({ baseUrl }).getBriefing()).rejects.toMatchObject({
      code: "unavailable",
      message: "Luna is temporarily unavailable. Your Household Work is safe.",
    });
  });
});
