import { spawnSync } from "node:child_process";
import { browser, expect } from "@wdio/globals";
import * as OTPAuth from "otpauth";
import { Key } from "webdriverio";

type ManagedCanaryCredential = {
  email: string;
  password: string;
  totpSecret: string;
  householdId: string;
  projectRef: string;
  devicePin: string;
};

function loadCredential(): ManagedCanaryCredential {
  const credentialPath = process.env.LUNA_CANARY_CREDENTIAL_PATH;
  if (!credentialPath) throw new Error("The managed-canary credential path is required.");
  const script = [
    "$secure = Import-Clixml -LiteralPath $env:LUNA_CANARY_CREDENTIAL_PATH",
    "$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)",
    "try { [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer) }",
  ].join("; ");
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
    encoding: "utf8",
    env: { ...process.env, LUNA_CANARY_CREDENTIAL_PATH: credentialPath },
    windowsHide: true,
  });
  if (result.status !== 0 || !result.stdout) {
    throw new Error("The managed-canary credential could not be opened.");
  }
  const credential = JSON.parse(result.stdout.trim()) as ManagedCanaryCredential;
  if (
    !credential.email
    || !credential.password
    || !credential.totpSecret
    || !credential.householdId
    || !credential.projectRef
    || !/^\d{6,}$/.test(credential.devicePin)
  ) {
    throw new Error("The managed-canary credential is incomplete.");
  }
  return credential;
}

function currentAuthenticatorCode(secret: string): string {
  return new OTPAuth.TOTP({
    algorithm: "SHA1",
    digits: 6,
    period: 30,
    secret: OTPAuth.Secret.fromBase32(secret),
  }).generate();
}

async function waitForAccountEntry() {
  await browser.waitUntil(async () => (
    await $("#organiser-name").isExisting()
    || await $("#sign-in-email").isExisting()
    || await $("#device-unlock-pin").isExisting()
    || await $("button[aria-label='Luna']").isExisting()
  ), { timeout: 30_000, timeoutMsg: "Luna did not reach account entry or an unlocked Household." });
}

async function signInIfNeeded(credential: ManagedCanaryCredential) {
  await waitForAccountEntry();
  if (await $("#device-unlock-pin").isExisting()) {
    await $("#device-unlock-pin").setValue(credential.devicePin);
    await $("button=Unlock Luna").click();
    return;
  }
  if (await $("button[aria-label='Luna']").isExisting()) return;
  if (await $("#organiser-name").isExisting()) await $("button=Sign in").click();
  await $("#sign-in-email").setValue(credential.email);
  await $("#sign-in-password").setValue(credential.password);
  await $("button=Sign in").click();
  await $("#sign-in-authenticator-code").waitForDisplayed({ timeout: 30_000 });
  await $("#sign-in-authenticator-code").setValue(currentAuthenticatorCode(credential.totpSecret));
  await $("button=Continue to Luna").click();
}

async function enrollFirstTrustedDeviceIfNeeded(credential: ManagedCanaryCredential) {
  await browser.waitUntil(async () => (
    await $("button=Continue trusted device setup").isExisting()
    || await $("#device-unlock-pin").isExisting()
    || await $("h1=Give Luna a desk").isExisting()
    || await $("button[aria-label='Luna']").isExisting()
    || await $("button=Use Recovery Key").isExisting()
  ), { timeout: 30_000, timeoutMsg: "Luna did not resolve Trusted Device enrollment." });
  if (await $("button=Use Recovery Key").isExisting()) {
    throw new Error("The canary Household has a remote device but this isolated canary has no matching local keys.");
  }
  if (await $("#device-unlock-pin").isExisting()) {
    await $("#device-unlock-pin").setValue(credential.devicePin);
    await $("button=Unlock Luna").click();
    return;
  }
  if (!await $("button=Continue trusted device setup").isExisting()) return;
  await $("button=Continue trusted device setup").click();
  await $("#recovery-key").waitForDisplayed({ timeout: 30_000 });
  const recoveryKey = await $("#recovery-key").getText();
  await $("#recovery-key-confirmation").setValue(recoveryKey);
  await $("button=Confirm Recovery Key").click();
  await $("#device-pin").setValue(credential.devicePin);
  await $("#device-pin-confirmation").setValue(credential.devicePin);
  await $("button=Save device PIN").click();
}

async function configureCabinetIfNeeded() {
  await browser.waitUntil(async () => (
    await $("h1=Give Luna a desk").isExisting()
    || await $("button[aria-label='Luna']").isExisting()
  ), { timeout: 30_000, timeoutMsg: "Luna did not reach Cabinet setup or the Household desk." });
  if (!await $("h1=Give Luna a desk").isExisting()) return;
  await $("button=Choose cabinet folder").click();
  await $("button=Create cabinet").click();
  await $("button[aria-label='Luna']").waitForDisplayed({ timeout: 30_000 });
}

describe("Luna managed-access live canary", () => {
  it("enables Conversation permission and receives a real managed AI reply", async () => {
    const credential = loadCredential();
    await signInIfNeeded(credential);
    await enrollFirstTrustedDeviceIfNeeded(credential);
    await configureCabinetIfNeeded();

    await $("button[aria-label='Options']").click();
    await $("button[aria-label='Cloud assistance options']").click();
    try {
      await browser.waitUntil(async () => (await $("body").getText()).includes("Managed access ready"), {
        timeout: Number(process.env.LUNA_LIVE_CANARY_READY_TIMEOUT_MS ?? "150000"),
        interval: 2_000,
        timeoutMsg: "Managed access did not become ready on the canary Trusted Device.",
      });
    } catch {
      const managedStatus = await $(".cloud-provider-list .cloud-provider-heading small").getText()
        .catch(() => "status unavailable");
      const visibleAlerts = await $$('[role="alert"], .session-notice').map((element) => element.getText());
      const synchronization = await browser.executeAsync((done) => {
        const canary = (window as unknown as {
          __LUNA_LIVE_CANARY__?: { synchronizeManagedAccess(): Promise<unknown> };
        }).__LUNA_LIVE_CANARY__;
        if (!canary) {
          done({ ok: false, name: "CanaryBridgeUnavailable", message: "Live canary bridge unavailable.", status: null });
          return;
        }
        void canary.synchronizeManagedAccess().then(done);
      }) as { ok: boolean; name?: string; message?: string; status?: number | null };
      throw new Error([
        `Managed access state: ${managedStatus}.`,
        synchronization.ok
          ? "Direct synchronization succeeded after the automatic attempt."
          : `Synchronization failure: ${synchronization.name} (${synchronization.status ?? "no status"}) ${synchronization.message}`,
        ...visibleAlerts.map((alert) => `Notice: ${alert}`),
      ].join(" "));
    }

    const defaultProvider = $("select[aria-label='Default Intelligence Provider and model']");
    await defaultProvider.selectByAttribute("value", "openai::gpt-4.1-mini");
    await $("button=Save default").click();
    await expect($("section[aria-label='Default intelligence settings']")).toHaveText(
      expect.stringContaining("OpenAI"),
    );
    const conversationPermission = $("input[aria-label='Allow Conversation replies by default']");
    if (!await conversationPermission.isSelected()) await conversationPermission.click();
    await expect(conversationPermission).toBeChecked();

    await $("button[aria-label='Luna']").click();
    await $("button[aria-label='New conversation']").click();
    const prompt = "Reply with one short sentence confirming Luna managed intelligence is available.";
    await $("#message-composer").setValue(prompt);
    await browser.keys(Key.Enter);
    await expect($(".member-message p")).toHaveText(prompt);
    await browser.waitUntil(async () => (
      await $(".luna-message .conversation-copy").isExisting()
      || await $(".conversation-intelligence-notice").isExisting()
      || await $("[role='alert']").isExisting()
    ), {
      timeout: 150_000,
      interval: 1_000,
      timeoutMsg: "The managed Conversation produced neither a reply nor a visible failure.",
    });
    if (await $(".conversation-intelligence-notice").isExisting()) {
      const audit = await browser.tauri.execute(({ core }, householdId) => (
        core.invoke<Array<{ providerId: string; modelId: string; capability: string; outcome: string; reason: string }>>(
          "list_cloud_assistance_audit_events",
          { householdId },
        )
      ), credential.householdId);
      const latestReply = [...audit].reverse().find((event) => event.capability === "conversationReply");
      throw new Error([
        `Managed Conversation failed: ${await $(".conversation-intelligence-notice").getText()}`,
        latestReply
          ? `Audit: ${latestReply.providerId}/${latestReply.modelId} ${latestReply.outcome} - ${latestReply.reason}`
          : "Audit: no Conversation event was recorded.",
      ].join(" "));
    }
    if (await $("[role='alert']").isExisting()) {
      throw new Error(`Managed Conversation failed: ${await $("[role='alert']").getText()}`);
    }
    const reply = await $(".luna-message .conversation-copy").getText();
    expect(reply.trim().length).toBeGreaterThan(0);

    const audit = await browser.tauri.execute(({ core }, householdId) => (
      core.invoke<Array<{ providerId: string; modelId: string; capability: string; outcome: string }>>(
        "list_cloud_assistance_audit_events",
        { householdId },
      )
    ), credential.householdId);
    const completedReply = [...audit].reverse().find((event) => (
      event.capability === "conversationReply" && event.outcome === "completed"
    ));
    expect(completedReply).toMatchObject({
      providerId: "openai",
      modelId: "gpt-4.1-mini",
      capability: "conversationReply",
      outcome: "completed",
    });
  });
});
