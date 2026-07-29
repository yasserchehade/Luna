import { browser, expect } from "@wdio/globals";
import "@wdio/tauri-service";
import { testHousehold } from "./testHousehold";

describe("Luna account access", () => {
  it("onboards an organiser, protects account privacy, and recovers access to the same Household", async () => {
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
    await expect($("#household-name")).toHaveValue("");

    await $("#household-name").setValue(testHousehold.name);
    await $("button=Create Household").click();

    await expect($("h1=Protect this trusted device")).toBeDisplayed();
    await $("button=Set up authenticator").click();
    await expect($("img[alt='Scan this QR code with your authenticator app']")).toBeDisplayed();
    await expect($("#authenticator-secret")).toHaveText(testHousehold.authenticatorSecret);
    await $("#authenticator-code").setValue(testHousehold.authenticatorCode);
    await $("button=Verify authenticator").click();

    await expect($("h1=Save your Recovery Key")).toBeDisplayed();
    const recoveryKey = await $("#recovery-key").getText();
    expect(recoveryKey.split(/\s+/)).toHaveLength(24);
    await $("#recovery-key-confirmation").setValue(recoveryKey);
    await $("button=Confirm Recovery Key").click();

    await expect($("h1=Create a device PIN")).toBeDisplayed();
    await $("#device-pin").setValue(testHousehold.devicePin);
    await $("#device-pin-confirmation").setValue(testHousehold.devicePin);
    await $("button=Save device PIN").click();

    await expect($("h1=Give Luna a desk")).toBeDisplayed();
    await expect($("button[aria-label='Cloud-synchronised storage']")).toHaveAttribute("aria-pressed", "true");
    await $("button[aria-label='Local or network storage']").click();
    await expect($("button[aria-label='Local or network storage']")).toHaveAttribute("aria-pressed", "true");
    await $("button[aria-label='Cloud-synchronised storage']").click();
    await $("button=Choose cabinet folder").click();
    await expect($("h1=Review your cabinet")).toBeDisplayed();
    await expect($$("input[name='cabinet-section']")).toBeElementsArrayOfSize(5);
    await $("input[aria-label='Cabinet section 1']").setValue("Household bills");
    await $("button[aria-label='Remove Identity']").click();
    await $("input[aria-label='New cabinet section']").setValue("Insurance");
    await $("button=Add section").click();
    const beforeConfirmation = await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("validate_cabinet", { householdId })
    ), testHousehold.id);
    expect(beforeConfirmation).toBeNull();
    await $("button=Create cabinet").click();

    await expect($("h1=New conversation")).toBeDisplayed();
    await $("button[aria-label='Conversation actions']").click();
    await expect($(`strong=${testHousehold.name}`)).toBeDisplayed();
    await $("button[aria-label='Conversation actions']").click();
    await expect($(`strong=${testHousehold.organiserName}`)).toBeDisplayed();
    await $("button[aria-label='Cabinet']").click();
    await expect($("h1=Cabinet")).toBeDisplayed();
    await expect($("strong=Incoming")).toBeDisplayed();
    await expect($("strong=Household bills")).toBeDisplayed();
    await expect($("strong=Insurance")).toBeDisplayed();
    await expect($("strong=Identity")).not.toBeExisting();
    await $("button[aria-label='Luna']").click();

    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(false);
    });
    await $("button=Lock Luna").click();
    await expect($("h1=Unlock this trusted device")).toBeDisplayed();
    await expect($("#sign-in-email")).not.toBeExisting();
    await expect($("#sign-in-authenticator-code")).not.toBeExisting();
    await $("#device-unlock-pin").setValue(testHousehold.devicePin);
    await $("button=Unlock Luna").click();

    await expect($("h1=New conversation")).toBeDisplayed();
    await expect($("[role='status']")).toHaveText(expect.stringContaining("working offline"));
    const offlineLayout = await browser.execute(() => {
      const sidebar = document.querySelector(".sidebar")?.getBoundingClientRect();
      const workspace = document.querySelector(".conversation-workspace")?.getBoundingClientRect();
      return {
        sidebarRight: sidebar?.right ?? 0,
        workspaceLeft: workspace?.left ?? 0,
        workspaceWidth: workspace?.width ?? 0,
      };
    });
    expect(offlineLayout.workspaceLeft).toBeGreaterThanOrEqual(offlineLayout.sidebarRight);
    expect(offlineLayout.workspaceWidth).toBeGreaterThan(offlineLayout.sidebarRight);
    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(true);
      window.dispatchEvent(new Event("online"));
    });
    await expect($("[role='status']")).not.toBeExisting();
    await $("button[aria-label='Conversation actions']").click();
    await expect($(`strong=${testHousehold.name}`)).toBeDisplayed();
    await $("button[aria-label='Conversation actions']").click();

    const recoveryCoordination = await browser.execute(() => {
      const control = (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: {
          currentRecovery(): { recoveryEnvelope: string; keyEpoch: number };
        };
      }).__LUNA_E2E_ACCOUNT__;
      return control.currentRecovery();
    });
    const currentDevicePublicKey = await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("current_device_public_key", { householdId })
    ), testHousehold.id) as string;
    const remoteRotation = await browser.tauri.execute(({ core }, request) => (
      core.invoke("prepare_household_key_rotation", request)
    ), {
      householdId: testHousehold.id,
      recoveryKey,
      recoveryEnvelope: recoveryCoordination.recoveryEnvelope,
      retainedDevicePublicKeys: [currentDevicePublicKey],
      currentKeyEpoch: recoveryCoordination.keyEpoch,
      revokedDeviceId: "00000000-0000-4000-8000-000000000099",
    }) as {
      deviceEnvelopes: Array<{ devicePublicKey: string; keyEnvelope: string }>;
      recoveryEnvelope: string;
    };
    await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("discard_household_key_rotation", { householdId })
    ), testHousehold.id);
    await browser.execute((rotation) => {
      const control = (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: {
          simulateRemoteRotation(request: typeof rotation): void;
        };
      }).__LUNA_E2E_ACCOUNT__;
      control.simulateRemoteRotation(rotation);
    }, {
      currentKeyEpoch: recoveryCoordination.keyEpoch,
      recoveryEnvelope: remoteRotation.recoveryEnvelope,
      deviceEnvelopes: remoteRotation.deviceEnvelopes,
    });

    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(false);
    });
    await $("button=Lock Luna").click();
    await expect($("h1=Unlock this trusted device")).toBeDisplayed();
    await $("#device-unlock-pin").setValue(testHousehold.devicePin);
    await $("button=Unlock Luna").click();
    await expect($("h1=New conversation")).toBeDisplayed();
    await expect($("[role='status']")).toHaveText(expect.stringContaining("working offline"));
    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(true);
      window.dispatchEvent(new Event("online"));
    });
    await expect($("[role='status']")).not.toBeExisting();
    await browser.waitUntil(async () => (
      await browser.tauri.execute(({ core }, householdId) => (
        core.invoke("current_key_epoch", { householdId })
      ), testHousehold.id) as number
    ) === 2, { timeout: 5_000, timeoutMsg: "the retained device did not apply its rotated key" });
    const protectedState = await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("protect_household_state", {
        householdId,
        plaintext: "state received after remote rotation",
      })
    ), testHousehold.id);
    const openedState = await browser.tauri.execute(({ core }, request) => (
      core.invoke("open_household_state", request)
    ), { householdId: testHousehold.id, protected: protectedState }) as string;
    expect(openedState).toBe("state received after remote rotation");

    await $("button[aria-label='Options']").click();
    await expect($("h1=Options")).toBeDisplayed();
    await expect($("h2=Trusted devices")).toBeDisplayed();
    const recoveryBeforeCancellation = await browser.execute(() => (
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: {
          currentRecovery(): { recoveryEnvelope: string; recoveryVerificationKey: string; keyEpoch: number };
        };
      }).__LUNA_E2E_ACCOUNT__.currentRecovery()
    ));
    await $("button=Replace lost Recovery Key").click();
    await $("#replacement-authenticator-code").setValue(testHousehold.authenticatorCode);
    await $("button=Verify and generate replacement").click();
    await expect($("h2=Save your replacement Recovery Key")).toBeDisplayed();
    await $("button=Cancel").click();
    await expect($("button=Replace lost Recovery Key")).toBeDisplayed();
    expect(await browser.execute(() => (
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: {
          currentRecovery(): { recoveryEnvelope: string; recoveryVerificationKey: string; keyEpoch: number };
        };
      }).__LUNA_E2E_ACCOUNT__.currentRecovery()
    ))).toEqual(recoveryBeforeCancellation);

    await $("button=Replace lost Recovery Key").click();
    await expect($("h2=Verify Recovery Key Replacement")).toBeDisplayed();
    await $("#replacement-authenticator-code").setValue(testHousehold.authenticatorCode);
    await $("button=Verify and generate replacement").click();
    await expect($("h2=Save your replacement Recovery Key")).toBeDisplayed();
    const replacementRecoveryKey = await $("#replacement-recovery-key-output").getText();
    expect(replacementRecoveryKey.split(/\s+/)).toHaveLength(24);
    expect(replacementRecoveryKey).not.toBe(recoveryKey);
    await $("#replacement-recovery-key-confirmation").setValue("wrong recovery key");
    await $("button=Confirm replacement Recovery Key").click();
    await expect($("[role='alert']")).toHaveText(expect.stringContaining("does not match"));
    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: {
          failAfterNextRecoveryReplacementCommit(): void;
          setCoordinationAvailable(available: boolean): void;
        };
      }).__LUNA_E2E_ACCOUNT__.failAfterNextRecoveryReplacementCommit();
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(false);
    });
    await $("#replacement-recovery-key-confirmation").setValue(replacementRecoveryKey);
    await $("button=Confirm replacement Recovery Key").click();
    await expect($("[role='alert']")).toHaveText(expect.stringContaining("could not confirm whether"));
    await expect($("#replacement-recovery-key-output")).toHaveText(replacementRecoveryKey);
    await expect($("button=Check replacement status")).toBeDisplayed();
    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { setCoordinationAvailable(available: boolean): void };
      }).__LUNA_E2E_ACCOUNT__.setCoordinationAvailable(true);
    });
    await $("button=Check replacement status").click();
    await expect($("[role='status']")).toHaveText(expect.stringContaining("previous Recovery Key no longer works"));

    await browser.execute(() => {
      (window as typeof window & {
        __LUNA_E2E_ACCOUNT__: { failNextAccountSignOut(): void };
      }).__LUNA_E2E_ACCOUNT__.failNextAccountSignOut();
    });
    await $("button=Lock Luna").click();
    await $("button=Sign out on this device").click();
    await expect($("h1=Finish signing out")).toBeDisplayed();
    await expect($(`strong=${testHousehold.name}`)).not.toBeExisting();
    await $("button=Retry sign out").click();
    await expect($("h1=Sign in to Luna")).toBeDisplayed();
    await $("#sign-in-email").setValue("unknown@example.com");
    await $("#sign-in-password").setValue("wrong-password-7");
    await $("button=Sign in").click();
    const unknownAccountMessage = await $("[role='alert']").getText();

    await $("#sign-in-email").setValue(testHousehold.email);
    await $("#sign-in-password").setValue("wrong-password-7");
    await $("button=Sign in").click();
    await expect($("[role='alert']")).toHaveText(unknownAccountMessage);

    await $("button=Forgot password?").click();
    await expect($("h1=Reset your password")).toBeDisplayed();
    await $("#recovery-email").setValue(testHousehold.email);
    const sendRecoveryCode = $("button=Send recovery code");
    await sendRecoveryCode.click();
    await sendRecoveryCode.click();

    await expect($("h1=Check your email")).toBeDisplayed();
    await expect($("[role='alert']")).not.toBeExisting();
    await expect($(`p=We sent a recovery code to ${testHousehold.email}.`)).toBeDisplayed();
    await $("#recovery-code").setValue(testHousehold.recoveryCode);
    await $("#replacement-password").setValue(testHousehold.replacementPassword);
    await $("#recovery-authenticator-code").setValue(testHousehold.authenticatorCode);
    await $("button=Set new password").click();

    await expect($("h1=Sign in to Luna")).toBeDisplayed();
    await expect($("[role='status']")).toHaveText("Your password has been changed. Sign in with your new password.");
    await browser.tauri.execute(({ core }, householdId) => (
      core.invoke("forget_current_device", { householdId })
    ), testHousehold.id);
    await $("#sign-in-email").setValue(testHousehold.email);
    await $("#sign-in-password").setValue(testHousehold.replacementPassword);
    await $("button=Sign in").click();
    await expect($("h1=Verify your identity")).toBeDisplayed();
    await $("button=Back to sign in").click();
    await expect($("h1=Sign in to Luna")).toBeDisplayed();
    await $("#sign-in-email").setValue(testHousehold.email);
    await $("#sign-in-password").setValue(testHousehold.replacementPassword);
    await $("button=Sign in").click();
    await expect($("h1=Verify your identity")).toBeDisplayed();
    await $("#sign-in-authenticator-code").setValue(testHousehold.authenticatorCode);
    await $("button=Continue to Luna").click();

    await expect($("h1=Recover this trusted device")).toBeDisplayed();
    await $("button=Use Recovery Key").click();
    await expect($("h1=Enter your Recovery Key")).toBeDisplayed();
    await $("#replacement-recovery-key").setValue(recoveryKey);
    await $("button=Recover trusted device").click();
    await expect($("[role='alert']")).toHaveText(expect.stringContaining("does not match"));
    await $("#replacement-recovery-key").setValue(replacementRecoveryKey);
    await $("button=Recover trusted device").click();
    await expect($("h1=Create a device PIN")).toBeDisplayed();
    await $("#device-pin").setValue(testHousehold.replacementDevicePin);
    await $("#device-pin-confirmation").setValue(testHousehold.replacementDevicePin);
    await $("button=Save device PIN").click();
    await expect($("button=Options")).toBeDisplayed();
    await $("button[aria-label='Conversation actions']").click();
    await expect($("strong=Rivera Household")).toBeDisplayed();
    await $("button[aria-label='Conversation actions']").click();

    await $("button[aria-label='Options']").click();
    await expect($("h1=Options")).toBeDisplayed();
    await expect($("h2=Trusted devices")).toBeDisplayed();
    await expect($("[data-device-label='This device']")).toHaveText(expect.stringContaining("Active"));
    await expect($("[data-device-label='Recovered device']")).toHaveText(expect.stringContaining("This device"));
    await $("button[aria-label='Revoke This device']").click();
    await expect($("h2=Confirm device revocation")).toBeDisplayed();
    await $("#revocation-recovery-key").setValue(replacementRecoveryKey);
    await $("button=Revoke device").click();
    await expect($("[data-device-label='This device']")).toHaveText(expect.stringContaining("Revoked"));

    await $("button=Sign out on this device").click();
    await $("button=Forgot password?").click();
    await $("#recovery-email").setValue("unknown@example.com");
    await $("button=Send recovery code").click();

    await expect($("h1=Check your email")).toBeDisplayed();
    await expect($("[role='alert']")).not.toBeExisting();
    await expect($("p=We sent a recovery code to unknown@example.com.")).toBeDisplayed();
  });
});
