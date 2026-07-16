import { $, browser } from "@wdio/globals";
import { testHousehold } from "./testHousehold";

export async function onboardTestHousehold() {
  const accountEntryAvailable = await browser.waitUntil(async () => (
    await $("#organiser-name").isExisting()
    || await $("#sign-in-email").isExisting()
  ), { timeout: 10_000 }).catch(() => false);
  if (!accountEntryAvailable) {
    const visibleContent = await $("body").getText().catch(() => "<not available>");
    throw new Error(`Luna account entry did not become available. Visible content: ${visibleContent}`);
  }

  if (await $("#organiser-name").isExisting()) {
    await registerTestHousehold();
    await configureCabinetIfNeeded();
    return;
  }

  await $("#sign-in-email").setValue(testHousehold.email);
  await $("#sign-in-password").setValue(testHousehold.password);
  await $("button=Sign in").click();

  if (await $("button=Create account").isExisting()) {
    await $("button=Create account").click();
    await registerTestHousehold();
  }
  await configureCabinetIfNeeded();
}

async function registerTestHousehold() {
  await $("#organiser-name").setValue(testHousehold.organiserName);
  await $("#account-email").setValue(testHousehold.email);
  await $("#account-password").setValue(testHousehold.password);
  await $("button=Create account").click();
  await $("#verification-code").setValue(testHousehold.verificationCode);
  await $("button=Verify email").click();
  await $("#household-name").setValue(testHousehold.name);
  await $("button=Create Household").click();
  await $("button=Set up authenticator").click();
  await $("#authenticator-code").setValue(testHousehold.authenticatorCode);
  await $("button=Verify authenticator").click();
  const recoveryKey = await $("#recovery-key").getText();
  await $("#recovery-key-confirmation").setValue(recoveryKey);
  await $("button=Confirm Recovery Key").click();
  await $("#device-pin").setValue(testHousehold.devicePin);
  await $("#device-pin-confirmation").setValue(testHousehold.devicePin);
  await $("button=Save device PIN").click();
}

async function configureCabinetIfNeeded() {
  await browser.waitUntil(async () => (
    await $("h1=Give Luna a desk").isExisting()
    || await $("button[aria-label='Luna']").isExisting()
  ), { timeoutMsg: "Luna desk setup did not become available" });
  if (!await $("h1=Give Luna a desk").isExisting()) return;
  await $("button=Choose cabinet folder").click();
  await $("button=Create cabinet").click();
  await $("button[aria-label='Luna']").waitForDisplayed();
}
