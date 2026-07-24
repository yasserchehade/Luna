import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { onboardTestHousehold } from "./onboardTestHousehold";

const enterKey = "\uE007";

describe("Luna Conversation desk", () => {
  it("sends a message with Enter and keeps Shift+Enter as a new line", async () => {
    await onboardTestHousehold();
    const composer = await $("#message-composer");
    await composer.click();
    await composer.setValue("First line");
    const shiftEnterAllowsNativeEdit = await browser.execute(() => {
      const composer = document.querySelector("#message-composer");
      if (!composer) return false;
      return composer.dispatchEvent(new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        key: "Enter",
        shiftKey: true,
      }));
    });

    expect(shiftEnterAllowsNativeEdit).toBe(true);
    await expect($$(".member-message")).toBeElementsArrayOfSize(0);
    await composer.addValue("\nSecond line");
    await expect(composer).toHaveValue("First line\nSecond line");

    await browser.keys(enterKey);

    await expect($$(".member-message")).toBeElementsArrayOfSize(1);
    await expect($(".member-message p")).toHaveText("First line\nSecond line");
    await expect(composer).toHaveValue("");
  });

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

    await expect($(".member-message:last-of-type p")).toHaveText(message);

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
    await expect($(".document-attachment strong")).toHaveText(expect.stringContaining("luna-e2e-document"));
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("This appears to be an electricity bill from AGL for 12 Seabreeze Avenue on account 12345678."),
    );
    await expect($(".context-review-form")).not.toBeDisplayed();
    await expect($(".filing-decision-form")).not.toBeDisplayed();
    await $(".review-details summary").click();
    await expect($("label=Service provider relevance")).toBeDisplayed();
    await expect($("label=Property relevance")).toBeDisplayed();
    const reviewCard = $(".document-arrival .review-card");
    await expect(reviewCard.$$("input[aria-label='Amount']")).toBeElementsArrayOfSize(1);
    const cloudAssistance = $("section[aria-label='Cloud assistance for this document']");
    await expect(cloudAssistance.$("button=Ask a provider")).toBeDisplayed();
    await cloudAssistance.$("button=Ask a provider").click();
    await expect(cloudAssistance.$("button=Keep local")).toBeDisplayed();
    await cloudAssistance.$("button=Keep local").click();
    await expect(cloudAssistance).toHaveText(expect.stringContaining("Kept local"));
    const clarificationPromptElements = await reviewCard.$$(".clarification-questions p");
    const clarificationPrompts: string[] = [];
    for (const prompt of clarificationPromptElements) clarificationPrompts.push(await prompt.getText());
    expect(clarificationPrompts.some((prompt) => prompt.toLowerCase().includes("amount"))).toBe(false);
    await $(".review-details summary").click();

    await $("button[aria-label='To do']").click();
    await expect($("h1=To do")).toBeDisplayed();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $("button=Open Conversation item").click();
    await expect($("h1=AGL electricity bill")).toBeDisplayed();
    await expect($(".document-arrival[data-focused='true']")).toBeDisplayed();

    const answerLuna = async (answer: string) => {
      const currentPrompt = await $(".document-luna-message .conversation-copy").getText();
      await $("#message-composer").setValue(answer);
      await $("button[aria-label='Send message']").click();
      await browser.waitUntil(
        async () => await $(".document-luna-message .conversation-copy").getText() !== currentPrompt,
        { timeoutMsg: `Luna did not advance after: ${answer}` },
      );
    };
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("I can file it in:"),
    );
    await answerLuna("Yes, that's right.");
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("Done. I filed the verified Original in:"),
    );
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("Bills & Services/12 Seabreeze Avenue/AGL/2026/"),
    );
    await $("button=Always do this").click();
    await browser.waitUntil(
      async () => !(await $("button=Always do this").isExisting()),
      { timeoutMsg: "The Filing Rule direction was not recorded." },
    );
    await $(".review-details summary").click();
    await expect($("[aria-label='Learned filing rule']")).toBeDisplayed();
    await expect($(".review-transparency")).toHaveText(expect.stringContaining("Member Direction"));
    await expect($(".review-transparency")).toHaveText(expect.stringContaining("Member chose Keep local"));
    await expect($(".review-transparency")).toHaveText(expect.stringContaining("Verified Original filed"));
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
    await $("button[aria-label='Cabinet']").click();
    await expect($(".filed-originals strong")).toHaveText(
      "2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf",
    );
    await expect($(".filed-originals p")).toHaveText(
      "Bills & Services/12 Seabreeze Avenue/AGL/2026/2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf",
    );
    await $("button[aria-label='History']").click();
    const filingHistory = $(".history-event:not(.cloud-history-event):not(.duplicate-history-event):not(.rule-history-event)");
    await expect(filingHistory.$("strong")).toHaveText("Document filed");
    await expect(filingHistory.$("small")).toHaveText(expect.stringContaining("2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf"));

    await $("button[aria-label='Options']").click();
    await expect($("h1=Options")).toBeDisplayed();
    await $("button[aria-label='Learned Filing Rules options']").click();
    await expect($("h2=Learned Filing Rules")).toBeDisplayed();
    await expect($("section[aria-label='Learned Filing Rules']")).toBeDisplayed();
    const learnedRule = $(".filing-rule-card");
    await expect(learnedRule).toHaveText(expect.stringContaining("Electricity bill from AGL"));
    await learnedRule.$("button=Edit rule").click();
    await $("button=Preview historical impact").click();
    await expect($("section[aria-label='Historical Filing Rule preview']")).toHaveText(expect.stringContaining("2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf"));
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
    const arrivalHeadings = await $$(".document-attachment strong");
    await expect(arrivalHeadings[0]).toHaveText(expect.stringContaining("luna-e2e-document"));
    await expect(arrivalHeadings[1]).toHaveText(expect.stringContaining("luna-e2e-dropped"));
    const droppedReviewDetails = $(".document-arrival[data-focused='true'] .review-details");
    await droppedReviewDetails.$("summary").click();
    await droppedReviewDetails.$("input[aria-label='Document type']").setValue("Household image");
    await droppedReviewDetails.$("input[aria-label='Service Provider']").setValue("Household Service");
    await droppedReviewDetails.$("input[aria-label='Service Provider relevance']").setValue("Maintains this household record");
    await droppedReviewDetails.$("input[aria-label='Addressee']").setValue("Sam Rivera");
    await droppedReviewDetails.$("input[aria-label='Account']").setValue("IMAGE-001");
    await droppedReviewDetails.$("button=Save Household Context").click();
    await expect(droppedReviewDetails.$("input[aria-label='Proposed filename']")).toBeDisplayed();
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $(".todo-list article").$("button=Dismiss").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
    await $("button[aria-label='Luna']").click();
    await expect($$(".document-attachment strong")).toBeElementsArrayOfSize(2);
    await expect($$(".document-luna-message .conversation-copy")[0]).toHaveText(
      expect.stringContaining("Done. I filed the verified Original"),
    );

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
    await $(".document-arrival[data-focused='true']").$("button=Discard new").click();
    await $("button[aria-label='To do']").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");

    await $("button[aria-label='New conversation']").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    const matchingDocument = await browser.tauri.execute(({ core }) => (
      core.invoke("select_e2e_context_document_file", { kind: "rule-match" })
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
    await expect($(".document-arrival[data-focused='true'] .conversation-copy")).toHaveText(
      expect.stringContaining("Done. I filed the verified Original"),
    );
    await $("button[aria-label='History']").click();
    await expect($(".history-event:not(.cloud-history-event):not(.duplicate-history-event):not(.rule-history-event) strong")).toHaveText("Automatically filed by learned rule");
    await $("button[aria-label='Luna']").click();
    await expect($(".document-arrival .conversation-copy")).toHaveText(
      expect.stringContaining("Done. I filed the verified Original"),
    );
    await $(".review-details summary").click();
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
    await expect($(".document-arrival[data-focused='true'] .conversation-copy")).toHaveText(
      expect.stringContaining("Origin"),
    );
    await $("button[aria-label='To do']").click();
    await expect($$(".todo-list article")).toBeElementsArrayOfSize(1);
    await $(".todo-list article").$("button=Dismiss").click();
    await expect($(".empty-state")).toHaveText("Nothing needs your attention.");
  });

  it("asks the Household Organiser to resolve an exact duplicate before filing it", async () => {
    await onboardTestHousehold();
    await $("button[aria-label='Luna']").click();
    const attachMatchingDocument = async () => {
      const documentPath = await browser.tauri.execute(({ core }) => (
        core.invoke("select_e2e_context_document_file", { kind: "matching" })
      )) as string;
      await browser.execute((path) => (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke(command: string, args: object): Promise<void>;
          };
        }
      ).__TAURI_INTERNALS__.invoke("plugin:event|emit", {
        event: "tauri://drag-drop",
        payload: { paths: [path], position: { x: 40, y: 40 } },
      }), documentPath);
    };

    await attachMatchingDocument();
    await expect($(".document-arrival[data-focused='true'] .conversation-copy")).toBeDisplayed();
    await attachMatchingDocument();
    await $(".document-arrival[data-focused='true'] .review-details summary").click();
    await expect($("section[aria-label^='Duplicate review for']")).toBeDisplayed();
    await expect($("section[aria-label^='Duplicate review for']")).toHaveText(expect.stringContaining("Exact byte duplicate"));
    await expect($("button=Keep both")).toBeDisplayed();
    await expect($("button=Link copies")).toBeDisplayed();
    await expect($("button=Discard new")).toBeDisplayed();
    await expect($("button=Updated version")).toBeDisplayed();
    await $("button=Keep both").click();
    await $(".document-arrival[data-focused='true'] .review-details summary").click();
    await expect($("[aria-label='Duplicate resolution']")).toHaveText(expect.stringContaining("Kept both Originals"));
    await expect($(".document-arrival[data-focused='true'] .conversation-copy")).toBeDisplayed();
    await $("button[aria-label='History']").click();
    await expect($(".duplicate-history-event strong")).toHaveText("Duplicate decision recorded");
    await expect($(".duplicate-history-event small")).toHaveText(expect.stringContaining("kept both Originals"));
  });
});
