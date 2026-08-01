import assert from "node:assert/strict";
import test from "node:test";
import { buildConversationTimeline } from "./conversationTimeline";

test("keeps an attached document at its upload point before later linked replies", () => {
  const timeline = buildConversationTimeline(
    [
      { id: 11, author: "luna", linkedDocumentArrival: 7, body: "What account is this for?" },
      { id: 12, author: "member", linkedDocumentArrival: 7, body: "The rental property account." },
    ],
    [{ id: 7, originalName: "utility-bill.pdf" }],
  );

  assert.deepEqual(
    timeline.map((entry) => entry.kind === "arrival"
      ? `arrival:${entry.arrival.id}`
      : `message:${entry.message.id}`),
    ["arrival:7", "message:11", "message:12"],
  );
});

test("uses a durable attachment anchor without rendering it as a chat bubble", () => {
  const timeline = buildConversationTimeline(
    [
      { id: 20, author: "member", linkedDocumentArrival: null, body: "Before upload" },
      { id: 21, author: "attachment", linkedDocumentArrival: 8, body: "rates.pdf" },
      { id: 22, author: "member", linkedDocumentArrival: null, body: "After upload" },
    ],
    [{ id: 8, originalName: "rates.pdf" }],
  );

  assert.deepEqual(
    timeline.map((entry) => entry.kind === "arrival"
      ? `arrival:${entry.arrival.id}`
      : `message:${entry.message.id}`),
    ["message:20", "arrival:8", "message:22"],
  );
});
