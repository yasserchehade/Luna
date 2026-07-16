import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { onboardTestHousehold } from "./onboardTestHousehold";

describe("Luna Conversation desk", () => {
  it("keeps document work consistent between Conversation and To do", async () => {
    await onboardTestHousehold();

    for (const destination of ["Luna", "To do", "Cabinet", "History", "Options"]) {
      await expect($(`button[aria-label='${destination}']`)).toBeDisplayed();
    }

    const message = "Please organise this electricity bill.";
    const composer = await $("#message-composer");
    await composer.setValue(message);
    await $("button[aria-label='Send message']").click();

    await expect($(".member-message p")).toHaveText(message);

    await $("button=Rename").click();
    const title = await $("input[aria-label='Conversation title']");
    await title.setValue("AGL electricity bill");
    await $("button=Save title").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();

    await $("button[aria-label='Attach document']").click();
    await expect($(".document-arrival h2")).toHaveText(expect.stringContaining("luna-e2e-document"));
    await expect($(".document-arrival p")).toHaveText("Needs your direction");

    await $("button[aria-label='To do']").click();
    await expect($("h1=To do")).toBeDisplayed();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $("button=Open Conversation item").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();

    await $(".document-arrival").$("button=Dismiss").click();
    await expect($(".document-arrival p")).toHaveText("Dismissed");
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");

    await $("button[aria-label='Luna']").click();
    const droppedDocument = join(tmpdir(), `luna-e2e-dropped-${Date.now()}.png`);
    writeFileSync(droppedDocument, Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
      "base64",
    ));
    await browser.execute((documentPath) => (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke(command: string, args: object): Promise<void>;
        };
      }
    ).__TAURI_INTERNALS__.invoke("plugin:event|emit", {
      event: "tauri://drag-drop",
      payload: { paths: [documentPath], position: { x: 40, y: 40 } },
    }), droppedDocument);
    await browser.waitUntil(async () => {
      const arrivals = await $$(".document-arrival");
      return await arrivals.length === 2 || await $("[role='alert']").isExisting();
    }, { timeoutMsg: "The dropped document was neither attached nor rejected." });
    const attachmentError = $("[role='alert']");
    if (await attachmentError.isExisting()) {
      throw new Error(`The dropped document was rejected: ${await attachmentError.getText()}`);
    }
    await expect($$(".document-arrival")).toBeElementsArrayOfSize(2);
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $(".todo-list article").$("button=Dismiss").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
    await $("button[aria-label='Luna']").click();
    await expect($$(".document-arrival > div > p")).toBeElementsArrayOfSize(2);
    for (const state of await $$(".document-arrival > div > p")) await expect(state).toHaveText("Dismissed");

    await $("button=Archive").click();
    await expect($("button=Restore")).toBeDisplayed();
    await $("label=Show archived").$("input").click();
    await expect($("h1=Conversations")).toBeDisplayed();
    await $("label=Show archived").$("input").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();

    const search = await $("input[aria-label='Search Conversations']");
    await search.setValue("No matching Conversation");
    await expect($("h1=Conversations")).toBeDisplayed();
    await search.setValue("AGL");
    await expect($("h1=AGL electricity bill")).toBeDisplayed();
    await $("button=Restore").click();

    await $("button=＋ New Conversation").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    await $("button=Delete").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();

    await $("button[aria-label='Attach document']").click();
    await $("button=Delete").click();
    await expect($("h1=Deleted Conversation")).toBeDisplayed();
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $("button=Open Conversation item").click();
    await expect($("h1=Deleted Conversation")).toBeDisplayed();
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();
    await $(".document-arrival[data-focused='true']").$("button=Dismiss").click();
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
  });
});
