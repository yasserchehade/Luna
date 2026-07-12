import { expect } from "@wdio/globals";
import { testHousehold } from "./testHousehold";

describe("Luna account onboarding", () => {
  it("creates a verified account and returns its organiser to the same Household", async () => {
    await expect($("h1=Create your Luna account")).toBeDisplayed();

    await $("button=Sign in").click();
    await expect($("h1=Sign in to Luna")).toBeDisplayed();
    await $("button=Create account").click();
    await expect($("h1=Create your Luna account")).toBeDisplayed();

    await $("#organiser-name").setValue(testHousehold.organiserName);
    await $("#account-email").setValue(testHousehold.email);
    await $("#account-password").setValue(testHousehold.password);
    await $("button=Create account").click();

    await expect($("h1=Check your email")).toBeDisplayed();
    await expect($(`p=We sent a verification code to ${testHousehold.email}.`)).toBeDisplayed();

    await $("#verification-code").setValue(testHousehold.verificationCode);
    await $("button=Verify email").click();

    await expect($("h1=Create your Household")).toBeDisplayed();

    await $("#household-name").setValue(testHousehold.name);
    await $("button=Create Household").click();

    await expect($("h1=New conversation")).toBeDisplayed();
    await expect($(`strong=${testHousehold.name}`)).toBeDisplayed();
    await expect($(`strong=${testHousehold.organiserName}`)).toBeDisplayed();

    await $("button=Sign out").click();
    await expect($("h1=Sign in to Luna")).toBeDisplayed();

    await $("#sign-in-email").setValue(testHousehold.email);
    await $("#sign-in-password").setValue(testHousehold.password);
    await $("button=Sign in").click();

    await expect($("h1=New conversation")).toBeDisplayed();
    await expect($(`strong=${testHousehold.name}`)).toBeDisplayed();

    await $("button=Sign out").click();
    await $("#sign-in-email").setValue("unknown@example.com");
    await $("#sign-in-password").setValue("wrong-password-7");
    await $("button=Sign in").click();
    const unknownAccountMessage = await $("[role='alert']").getText();

    await $("#sign-in-email").setValue(testHousehold.email);
    await $("#sign-in-password").setValue("wrong-password-7");
    await $("button=Sign in").click();
    await expect($("[role='alert']")).toHaveText(unknownAccountMessage);
  });
});
