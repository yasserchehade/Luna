import { expect } from "@wdio/globals";

describe("Luna conversation desk", () => {
  it("accepts work and shows it in Conversation", async () => {
    for (const destination of ["Luna", "To do", "Cabinet", "History", "Options"]) {
      await expect($(`button[aria-label='${destination}']`)).toBeDisplayed();
    }

    const message = "Please organise this electricity bill.";
    const composer = await $("#message-composer");
    await composer.setValue(message);
    await $("button[aria-label='Send message']").click();

    await expect($(".member-message p")).toHaveText(message);
  });
});
