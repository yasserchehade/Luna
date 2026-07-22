import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { onboardTestHousehold } from "./onboardTestHousehold";

describe("Luna Conversation desk", () => {
  it("keeps document work consistent between Conversation and To do", async () => {
    await onboardTestHousehold();
    const openConversationActions = async () => {
      await $("button[aria-label='Conversation actions']").click();
    };

    for (const destination of ["Luna", "To do", "Cabinet", "History", "Options"]) {
      await expect($(`button[aria-label='${destination}']`)).toBeDisplayed();
    }

    const message = "Please organise this electricity bill.";
    const composer = await $("#message-composer");
    await composer.setValue(message);
    await $("button[aria-label='Send message']").click();

    await expect($(".member-message p")).toHaveText(message);

    await openConversationActions();
    await $("button=Rename").click();
    const title = await $("input[aria-label='Conversation title']");
    await title.setValue("AGL electricity bill");
    await $("button=Save title").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();
    await expect($(".conversation-header")).toBeDisplayed();
    await expect($(".conversation-title")).toHaveText("AGL electricity bill");
    await expect($(".conversation-list")).toBeDisplayed();
    await expect($(".conversation-list")).toHaveText(expect.stringContaining("AGL electricity bill"));
    await expect((await $(".conversation-list button").getSize()).height).toBeLessThan(60);
    await $(".conversation-list button").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();

    await $("button[aria-label='Attach document']").click();
    await expect($(".document-arrival h2")).toHaveText(expect.stringContaining("luna-e2e-document"));
    await expect($(".document-arrival p")).toHaveText("Needs your direction");
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();
    await expect($("label=Service provider relevance")).toBeDisplayed();
    await expect($("label=Property relevance")).toBeDisplayed();
    const reviewCard = $(".document-arrival .review-card");
    await expect(reviewCard.$$("input[aria-label='Amount']")).toBeElementsArrayOfSize(1);
    const clarificationPromptElements = await reviewCard.$$(".clarification-questions p");
    const clarificationPrompts: string[] = [];
    for (const prompt of clarificationPromptElements) clarificationPrompts.push(await prompt.getText());
    expect(clarificationPrompts.some((prompt) => prompt.toLowerCase().includes("amount"))).toBe(false);

    await $("button[aria-label='To do']").click();
    await expect($("h1=To do")).toBeDisplayed();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $("button=Open Conversation item").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();

    const review = $(".document-arrival .review-card");
    await review.$("input[aria-label='Document type']").setValue("Electricity bill");
    await review.$("input[aria-label='Service Provider']").setValue("AGL");
    await review.$("input[aria-label='Service Provider relevance']").setValue(
      "Supplies electricity to our home",
    );
    await review.$("input[aria-label='Addressee']").setValue("Sam Rivera");
    await review.$("input[aria-label='Property address']").setValue("12 Seabreeze Avenue");
    await review.$("input[aria-label='Property relevance']").setValue("Our primary residence");
    await review.$("input[aria-label='Account']").setValue("12345678");
    await review.$("input[aria-label='Amount']").setValue("$184.72");
    await review.$("input[aria-label='Relevant dates']").setValue("2026-07-15, 2026-08-02");
    await review.$("button=Save Household Context").click();
    await expect(review.$("input[aria-label='Proposed filename']")).toHaveValue(
      "2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf",
    );
    await review.$("input[aria-label='Proposed filename']").setValue("AGL bill July 2026.pdf");
    await review.$("input[aria-label='Cabinet Destination']").setValue(
      "Bills & Services/12 Seabreeze Avenue/AGL/2026/AGL bill July 2026.pdf",
    );
    await review.$("button=Confirm Filing Decision").click();
    await expect($(".document-arrival > div > p")).toHaveText("Filed");
    await expect($("[aria-label='Learned filing rule']")).toBeDisplayed();
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
    await $("button[aria-label='Cabinet']").click();
    await expect($(".filed-originals strong")).toHaveText("AGL bill July 2026.pdf");
    await expect($(".filed-originals p")).toHaveText(
      "Bills & Services/12 Seabreeze Avenue/AGL/2026/AGL bill July 2026.pdf",
    );
    await $("button[aria-label='History']").click();
    await expect($(".history-event strong")).toHaveText("Document filed");
    await expect($(".history-event small")).toHaveText(expect.stringContaining("AGL bill July 2026.pdf"));

    await $("button[aria-label='Options']").click();
    await expect($("section[aria-label='Learned Filing Rules']")).toBeDisplayed();
    const learnedRule = $(".filing-rule-card");
    await expect(learnedRule).toHaveText(expect.stringContaining("Electricity bill from AGL"));
    await learnedRule.$("button=Edit rule").click();
    await $("button=Preview historical impact").click();
    await expect($("section[aria-label='Historical Filing Rule preview']")).toHaveText(expect.stringContaining("AGL bill July 2026.pdf"));
    await $("button=Cancel").click();
    await learnedRule.$("button=Pause rule").click();
    await expect(learnedRule).toHaveText(expect.stringContaining("Paused rule"));
    await learnedRule.$("button=Resume rule").click();
    await expect(learnedRule).toHaveText(expect.stringContaining("Active rule"));

    await $("button[aria-label='Luna']").click();
    await expect($(".attachment-zone p")).toHaveText(
      "Drop a PDF, JPG, or PNG anywhere in Luna, or select a document.",
    );
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
    const arrivalHeadings = await $$(".document-arrival h2");
    await expect(arrivalHeadings[0]).toHaveText(expect.stringContaining("luna-e2e-document"));
    await expect(arrivalHeadings[1]).toHaveText(expect.stringContaining("luna-e2e-dropped"));
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $(".todo-list article").$("button=Dismiss").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
    await $("button[aria-label='Luna']").click();
    await expect($$(".document-arrival > div > p")).toBeElementsArrayOfSize(2);
    await expect($$(".document-arrival > div > p")[0]).toHaveText("Filed");
    await expect($$(".document-arrival > div > p")[1]).toHaveText("Dismissed");

    await openConversationActions();
    await $("button=Archive").click();
    await expect($("h1=Conversations")).toBeDisplayed();
    await openConversationActions();
    await $("label=Show archived").$("input").click();
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

    await $("button[aria-label='New conversation']").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    await openConversationActions();
    await $("button=Delete").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();

    await $("button[aria-label='Attach document']").click();
    await openConversationActions();
    await $("button=Delete").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    const recoveredMessage = "The recovered conversation is writable.";
    await $("#message-composer").setValue(recoveredMessage);
    await $("button[aria-label='Send message']").click();
    await expect($(".member-message p")).toHaveText(recoveredMessage);
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $("button=Open Conversation item").click();
    await expect($("h1=Deleted Conversation")).toBeDisplayed();
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();
    await $(".document-arrival[data-focused='true']").$("button=Dismiss").click();
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");

    await $("button[aria-label='New conversation']").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    const matchingDocument = await browser.tauri.execute(({ core }) => (
      core.invoke("select_e2e_context_document_file", { kind: "matching" })
    )) as string;
    await browser.execute((documentPath) => (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke(command: string, args: object): Promise<void>;
        };
      }
    ).__TAURI_INTERNALS__.invoke("plugin:event|emit", {
      event: "tauri://drag-drop",
      payload: { paths: [documentPath], position: { x: 40, y: 40 } },
    }), matchingDocument);
    await expect($(".document-arrival[data-focused='true'] > div > p")).toHaveText("Filed");
    await $("button[aria-label='History']").click();
    await expect($(".history-event strong")).toHaveText("Automatically filed by learned rule");
    await $("button[aria-label='Luna']").click();
    await expect($(".document-arrival > div > p")).toHaveText("Filed");
    await expect($("[aria-label='Learned filing rule']")).toBeDisplayed();
    const changedDocument = await browser.tauri.execute(({ core }) => (
      core.invoke("select_e2e_context_document_file", { kind: "changed-provider" })
    )) as string;
    await browser.execute((documentPath) => (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke(command: string, args: object): Promise<void>;
        };
      }
    ).__TAURI_INTERNALS__.invoke("plugin:event|emit", {
      event: "tauri://drag-drop",
      payload: { paths: [documentPath], position: { x: 40, y: 40 } },
    }), changedDocument);
    await expect($(".document-arrival[data-focused='true'] > div > p")).toHaveText("Needs your direction");
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $(".todo-list article").$("button=Dismiss").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
  });
});
