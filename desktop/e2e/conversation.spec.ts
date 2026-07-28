import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { onboardTestHousehold } from "./onboardTestHousehold";
import { testHousehold } from "./testHousehold";

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
    await browser.execute(() => document.querySelector<HTMLTextAreaElement>("#message-composer")?.focus());
    expect(await browser.execute(() => document.activeElement?.id)).toBe("message-composer");

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
    const cloudAssistance = $("section[aria-label='Cloud assistance for this document']");
    await expect(cloudAssistance).toBeDisplayed();
    await expect(cloudAssistance).toHaveText(expect.stringContaining("OpenAI"));
    await expect(cloudAssistance).toHaveText(expect.stringContaining("4,000 characters"));
    await expect(cloudAssistance.$("button=Keep local")).toBeDisplayed();
    await browser.execute(() => document.querySelector<HTMLElement>(".review-details summary")?.focus());
    expect(await browser.execute(() => document.activeElement?.tagName)).toBe("SUMMARY");
    await $(".review-details summary").click();
    await expect($("label=Service provider relevance")).toBeDisplayed();
    await expect($("label=Property relevance")).toBeDisplayed();
    const reviewCard = $(".document-arrival .review-card");
    await cloudAssistance.$("button=Keep local").click();
    await expect(cloudAssistance).toHaveText(expect.stringContaining("Kept local"));
    await expect($("label=Service provider relevance")).toBeDisplayed();
    await expect($("label=Property relevance")).toBeDisplayed();
    await expect(reviewCard.$$("input[aria-label='Amount']")).toBeElementsArrayOfSize(1);
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
    await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("set_e2e_cabinet_availability", { householdId, available: false })
    ), testHousehold.id);
    try {
      await answerLuna("Yes, that's right.");
      await expect($(".document-arrival [role='status']")).toHaveText(expect.stringContaining("Cabinet is unavailable"));
      await $(".review-details summary").click();
      await expect($(".document-arrival .review-card").$("dt=Recovery status")).toBeDisplayed();
      await expect($(".document-arrival .review-card")).toHaveText(expect.stringContaining("only the confirmed Cabinet Destination"));
      await $(".review-details summary").click();
      await $("button[aria-label='To do']").click();
      await expect($(".todo-view header span")).toHaveText("1 requiring attention");
      await expect($(".todo-list article")).toHaveText(expect.stringContaining("Waiting for Cabinet"));
      await expect($(".todo-list article").$("button=Dismiss")).not.toExist();
      await $("button=Open Conversation item").click();
    } finally {
      await browser.tauri.execute(({ core }, householdId) => (
        core.invoke("set_e2e_cabinet_availability", { householdId, available: true })
      ), testHousehold.id);
      await browser.execute(() => window.dispatchEvent(new Event("online")));
    }
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("Done. I filed the verified Original in:"),
    );
    await expect($(".document-luna-message .conversation-copy")).toHaveText(
      expect.stringContaining("Bills & Services/12 Seabreeze Avenue/AGL/2026/"),
    );
    await browser.execute(() => Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
      .find((button) => button.textContent === "Always do this")?.focus());
    expect(await browser.execute(() => document.activeElement?.textContent)).toBe("Always do this");
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

  it("completes one-time, reusable, revoked, and local-only consent choices", async () => {
    await onboardTestHousehold();
    await $("button[aria-label='Luna']").click();
    await $("button[aria-label='New conversation']").click();

    const attach = async (kind: string) => {
      const arrivalSelector = ".messages > .document-arrival";
      const initialArrivalCount = await $$(arrivalSelector).length;
      const documentPath = await browser.tauri.execute(({ core }, selectedKind) => (
        core.invoke("select_e2e_context_document_file", { kind: selectedKind })
      ), kind) as string;
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
      await browser.waitUntil(async () => {
        const currentArrivalCount = await $$(arrivalSelector).length;
        return currentArrivalCount > initialArrivalCount
          || await $("[role='alert']").isExisting();
      }, {
        timeout: 60_000,
        timeoutMsg: `The ${kind} test document was neither attached nor rejected.`,
      });
      const attachmentError = $("[role='alert']");
      if (await attachmentError.isExisting()) {
        throw new Error(`The ${kind} test document was rejected: ${await attachmentError.getText()}`);
      }
      const arrivals = await $$(arrivalSelector);
      const arrivalCount = await arrivals.length;
      const arrival = arrivals[arrivalCount - 1];
      await expect(arrival).toBeDisplayed();
      const keepBoth = arrival.$("button=Keep both");
      if (await keepBoth.isExisting()) {
        await keepBoth.click();
      }
      const reviewCloudAssistance = arrival.$("button=Review Cloud Assistance");
      if (await reviewCloudAssistance.isExisting()) {
        await reviewCloudAssistance.click();
      }
      return arrival;
    };

    let arrival = await attach("cloud-scope");
    let assistance = arrival.$("section[aria-label='Cloud assistance for this document']");
    await expect(assistance).toHaveText(expect.stringContaining(
      "Reusable scope: future difficult application/pdf Documents with the same currently displayed local context values and disclosed fields.",
    ));
    await assistance.$("button=Allow this scoped future use").click();
    await expect(assistance).toHaveText(expect.stringContaining("suggested amount"));

    arrival = await attach("cloud-reuse");
    assistance = arrival.$("section[aria-label='Cloud assistance for this document']");
    await expect(assistance.$("button=Use existing Consent Grant")).toBeDisplayed();
    await assistance.$("button=Use existing Consent Grant").click();
    await expect(assistance).toHaveText(expect.stringContaining("suggested amount"));

    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setManagedIntelligenceState(state: "free" | "checkoutPending" | "provisioning" | "ready" | "paymentProblem" | "ended"): void };
      }).__LUNA_E2E_ACCOUNT__.setManagedIntelligenceState("free");
    });
    await $("button[aria-label='Options']").click();
    await $("button[aria-label='Cloud assistance options']").click();
    let cloudOptions = $("section[aria-label='Cloud assistance']");
    await expect(cloudOptions).toHaveText(expect.stringContaining("Managed access not included"));
    await cloudOptions.$("button=Start Paddle sandbox checkout").click();
    await expect(cloudOptions.$("a=Continue to Paddle sandbox")).toBeDisplayed();
    await expect(cloudOptions).toHaveText(expect.stringContaining("No real charge"));

    for (const [state, status] of [
      ["checkoutPending", "Checkout pending"],
      ["provisioning", "Preparing this Trusted Device"],
      ["paymentProblem", "Payment needs attention"],
      ["ended", "Managed access ended"],
    ] as const) {
      await browser.execute((nextState) => {
        (window as typeof window & {
          __LUNA_E2E_ACCOUNT__: { setManagedIntelligenceState(value: typeof nextState): void };
        }).__LUNA_E2E_ACCOUNT__.setManagedIntelligenceState(nextState);
      }, state);
      await $("button[aria-label='Learned Filing Rules options']").click();
      await $("button[aria-label='Cloud assistance options']").click();
      cloudOptions = $("section[aria-label='Cloud assistance']");
      await expect(cloudOptions).toHaveText(expect.stringContaining(status));
    }

    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setManagedIntelligenceState(state: "free" | "checkoutPending" | "provisioning" | "ready" | "paymentProblem" | "ended"): void };
      }).__LUNA_E2E_ACCOUNT__.setManagedIntelligenceState("ready");
    });
    await $("button[aria-label='Learned Filing Rules options']").click();
    await $("button[aria-label='Cloud assistance options']").click();
    cloudOptions = $("section[aria-label='Cloud assistance']");
    await expect(cloudOptions).toHaveText(expect.stringContaining("Managed access ready"));
    await expect(cloudOptions).toHaveText(expect.stringContaining("Complimentary beta"));
    await expect(cloudOptions).toHaveText(expect.stringContaining("You never need to enter a Luna access key"));
    const byokConnection = cloudOptions.$("section[aria-label='OpenAI bring-your-own-key connection']");
    await expect(byokConnection).toHaveText(expect.stringContaining("Not connected"));
    const providerKey = byokConnection.$("input[type='password']");
    await providerKey.setValue("sk-e2e-customer-provider-key");
    await byokConnection.$("button=Test and connect").click();
    await expect(byokConnection).toHaveText(expect.stringContaining("Connected"));
    await providerKey.setValue("sk-e2e-replacement-provider-key");
    await byokConnection.$("button=Test and replace").click();
    await expect(byokConnection).toHaveText(expect.stringContaining("Connected"));
    await byokConnection.$("button=Remove key").click();
    await expect(byokConnection).toHaveText(expect.stringContaining("Not connected"));
    const reusableGrant = $(".consent-scope-list li");
    await expect(reusableGrant).toHaveText(expect.stringContaining("Active"));
    await reusableGrant.$("button=Revoke").click();
    await expect(reusableGrant).toHaveText(expect.stringContaining("Revoked"));

    await $("button[aria-label='Luna']").click();
    arrival = await attach("cloud-once");
    assistance = arrival.$("section[aria-label='Cloud assistance for this document']");
    await assistance.$("button=Allow once").click();
    await expect(assistance).toHaveText(expect.stringContaining("suggested amount"));

    arrival = await attach("cloud-local");
    assistance = arrival.$("section[aria-label='Cloud assistance for this document']");
    await assistance.$("button=Keep local").click();
    await expect(assistance).toHaveText(expect.stringContaining("Kept local"));
  });
});
