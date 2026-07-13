import { $, browser } from "@wdio/globals";
import { testHousehold } from "./testHousehold";

export async function onboardTestHousehold() {
  await browser.waitUntil(async () => (
    await $("#organiser-name").isExisting()
    || await $("#sign-in-email").isExisting()
  ), { timeoutMsg: "Luna account entry did not become available" });

  if (await $("#organiser-name").isExisting()) {
    await registerTestHousehold();
    return;
  }

  await $("#sign-in-email").setValue(testHousehold.email);
  await $("#sign-in-password").setValue(testHousehold.password);
  await $("button=Sign in").click();

  if (!await $("button[aria-label='Luna']").isExisting()) {
    await $("button=Create account").click();
    await registerTestHousehold();
  }
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
}
