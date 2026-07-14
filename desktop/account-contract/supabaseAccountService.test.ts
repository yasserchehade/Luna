import assert from "node:assert/strict";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { createPrivateKey, createPublicKey, randomBytes, sign } from "node:crypto";
import test from "node:test";
import { createClient } from "@supabase/supabase-js";
import * as OTPAuth from "otpauth";
import { SupabaseAccountService } from "../src/account/supabaseAccountService";

const localConfig = readLocalSupabaseConfig();
const supabaseUrl = process.env.SUPABASE_URL ?? localConfig.API_URL;
const publishableKey = process.env.SUPABASE_PUBLISHABLE_KEY ?? localConfig.PUBLISHABLE_KEY;

test("a verified Luna Account returns to the same Household after signing in again", async () => {
  assert.ok(supabaseUrl, "SUPABASE_URL is required");
  assert.ok(publishableKey, "SUPABASE_PUBLISHABLE_KEY is required");

  const sessionStorage = memorySessionStorage();
  const accountService = new SupabaseAccountService(supabaseUrl, publishableKey, sessionStorage);
  const email = `sam.${crypto.randomUUID()}@example.com`;
  const password = "correct-horse-battery-staple-7";

  await accountService.register({ organiserName: "Sam Rivera", email, password });
  const verificationCode = await readLatestEmailCode(email);
  await accountService.verifyEmail(email, verificationCode);

  const created = await accountService.createHousehold("Rivera Household");
  assert.equal(created.organiserName, "Sam Rivera");
  assert.equal(created.householdName, "Rivera Household");
  const recoveryAuthority = createRecoveryAuthority();

  const authenticator = await accountService.beginAuthenticatorEnrollment();
  const totp = new OTPAuth.TOTP({
    issuer: "Luna",
    label: email,
    algorithm: "SHA1",
    digits: 6,
    period: 30,
    secret: OTPAuth.Secret.fromBase32(authenticator.secret),
  });
  await accountService.verifyAuthenticatorEnrollment(authenticator.factorId, totp.generate());
  assert.equal(await accountService.getAuthenticatorStatus(), "verified");
  const trustedDevice = await accountService.registerFirstTrustedDevice({
    label: "Sam's test PC",
    publicKey: "age1testdevicepublickey",
    keyEnvelope: "encrypted-device-envelope",
    recoveryEnvelope: "encrypted-recovery-envelope",
    recoveryVerificationKey: recoveryAuthority.verificationKey,
  });
  assert.equal(trustedDevice.label, "Sam's test PC");
  assert.equal(trustedDevice.status, "active");
  assert.deepEqual(await accountService.getTrustedDeviceRecoveryEnvelope(), {
    recoveryEnvelope: "encrypted-recovery-envelope",
    keyEpoch: 1,
  });

  const restartedAccountService = new SupabaseAccountService(
    supabaseUrl,
    publishableKey,
    sessionStorage,
  );
  assert.deepEqual(await restartedAccountService.restoreSession(), created);
  assert.equal(await restartedAccountService.getAuthenticatorStatus(), "verified");

  await accountService.signOut();
  const signedOutAccountService = new SupabaseAccountService(
    supabaseUrl,
    publishableKey,
    sessionStorage,
  );
  assert.equal(await signedOutAccountService.restoreSession(), null);
  const returned = await accountService.signIn(email, password);
  assert.equal(await accountService.getAuthenticatorStatus(), "challengeRequired");
  await accountService.verifyAuthenticatorChallenge(totp.generate());
  assert.equal(await accountService.getAuthenticatorStatus(), "verified");
  assert.deepEqual(returned, created);

  await accountService.signOut();
  await accountService.requestPasswordReset(email);
  const recoveryCode = await readLatestEmailCode(email);
  const replacementPassword = "replacement-horse-battery-staple-8";
  await accountService.resetPassword({
    email,
    recoveryCode,
    authenticatorCode: totp.generate(),
    newPassword: replacementPassword,
  });
  await assert.rejects(
    accountService.resetPassword({
      email,
      recoveryCode,
      authenticatorCode: totp.generate(),
      newPassword: "second-replacement-password-9",
    }),
    "a recovery code must not be reusable",
  );
  await accountService.signOut();
  await assert.rejects(accountService.signIn(email, password));
  assert.deepEqual(await accountService.signIn(email, replacementPassword), created);
  assert.equal(await accountService.getAuthenticatorStatus(), "challengeRequired");
  await accountService.verifyAuthenticatorChallenge(totp.generate());
  const recoveredPublicKey = "age1testreplacementdevicepublickey";
  const recoveredKeyEnvelope = "encrypted-replacement-device-envelope";
  const recoveredAuthorization = recoveryAuthority.authorize(recoverDeviceAuthorization(
    created.householdId,
    1,
    recoveredPublicKey,
    recoveredKeyEnvelope,
  ));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Unauthorised replacement",
    publicKey: recoveredPublicKey,
    keyEnvelope: recoveredKeyEnvelope,
    keyEpoch: 1,
    recoveryAuthorizationSignature: randomBytes(64).toString("base64"),
  }));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Missing recovery authority",
    publicKey: recoveredPublicKey,
    keyEnvelope: recoveredKeyEnvelope,
    keyEpoch: 1,
    recoveryAuthorizationSignature: null as unknown as string,
  }));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Substituted replacement envelope",
    publicKey: recoveredPublicKey,
    keyEnvelope: "substituted-replacement-device-envelope",
    keyEpoch: 1,
    recoveryAuthorizationSignature: recoveredAuthorization,
  }));
  const recoveredDevice = await accountService.registerRecoveredTrustedDevice({
    label: "Sam's replacement PC",
    publicKey: recoveredPublicKey,
    keyEnvelope: recoveredKeyEnvelope,
    keyEpoch: 1,
    recoveryAuthorizationSignature: recoveredAuthorization,
  });
  assert.equal(recoveredDevice.label, "Sam's replacement PC");
  assert.equal(recoveredDevice.keyEpoch, 1);
  assert.equal(recoveredDevice.status, "active");
  assert.equal((await accountService.listTrustedDevices()).length, 2);
  assert.deepEqual(await accountService.getTrustedDeviceKeyCoordination(trustedDevice.publicKey), {
    keyEnvelope: "encrypted-device-envelope",
    keyEpoch: 1,
    status: "active",
  });
  await assert.rejects(accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: "unauthorised-recovery-envelope",
    deviceEnvelopes: [{
      devicePublicKey: recoveredDevice.publicKey,
      keyEnvelope: "unauthorised-device-envelope",
    }],
    recoveryAuthorizationSignature: randomBytes(64).toString("base64"),
  }));
  await assert.rejects(accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: "missing-authority-recovery-envelope",
    deviceEnvelopes: [{
      devicePublicKey: recoveredDevice.publicKey,
      keyEnvelope: "missing-authority-device-envelope",
    }],
    recoveryAuthorizationSignature: null as unknown as string,
  }));
  const rotatedRecoveryEnvelope = "rotated-recovery-envelope";
  const rotatedDeviceEnvelopes = [{
    devicePublicKey: recoveredDevice.publicKey,
    keyEnvelope: "rotated-replacement-device-envelope",
  }];
  const revocationAuthorization = recoveryAuthority.authorize(revokeDeviceAuthorization(
    created.householdId,
    1,
    trustedDevice.id,
    recoveredDevice.publicKey,
    rotatedRecoveryEnvelope,
    rotatedDeviceEnvelopes,
  ));
  await assert.rejects(accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: "substituted-recovery-envelope",
    deviceEnvelopes: rotatedDeviceEnvelopes,
    recoveryAuthorizationSignature: revocationAuthorization,
  }));
  await assert.rejects(accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: rotatedRecoveryEnvelope,
    deviceEnvelopes: [{
      devicePublicKey: recoveredDevice.publicKey,
      keyEnvelope: "substituted-device-envelope",
    }],
    recoveryAuthorizationSignature: revocationAuthorization,
  }));
  const afterRevocation = await accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: rotatedRecoveryEnvelope,
    deviceEnvelopes: rotatedDeviceEnvelopes,
    recoveryAuthorizationSignature: revocationAuthorization,
  });
  assert.deepEqual(
    afterRevocation.map(({ label, keyEpoch, status }) => ({ label, keyEpoch, status })),
    [
      { label: "Sam's test PC", keyEpoch: 1, status: "revoked" },
      { label: "Sam's replacement PC", keyEpoch: 2, status: "active" },
    ],
  );
  assert.deepEqual(await accountService.getTrustedDeviceRecoveryEnvelope(), {
    recoveryEnvelope: "rotated-recovery-envelope",
    keyEpoch: 2,
  });
  assert.deepEqual(await accountService.getTrustedDeviceKeyCoordination(recoveredDevice.publicKey), {
    keyEnvelope: "rotated-replacement-device-envelope",
    keyEpoch: 2,
    status: "active",
  });
  assert.deepEqual(await accountService.getTrustedDeviceKeyCoordination(trustedDevice.publicKey), {
    keyEnvelope: "encrypted-device-envelope",
    keyEpoch: 1,
    status: "revoked",
  });
  await assert.rejects(accountService.revokeTrustedDevice({
    deviceId: trustedDevice.id,
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 1,
    recoveryEnvelope: "stale-recovery-envelope",
    deviceEnvelopes: [{
      devicePublicKey: recoveredDevice.publicKey,
      keyEnvelope: "stale-device-envelope",
    }],
    recoveryAuthorizationSignature: recoveryAuthority.authorize(revokeDeviceAuthorization(
      created.householdId,
      1,
      trustedDevice.id,
      recoveredDevice.publicKey,
      "stale-recovery-envelope",
      [{ devicePublicKey: recoveredDevice.publicKey, keyEnvelope: "stale-device-envelope" }],
    )),
  }));

  const organiserClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await organiserClient.auth.signInWithPassword({ email, password: replacementPassword })).error, null);
  const organiserHouseholds = await organiserClient.from("households").select("id");
  assert.equal(organiserHouseholds.error, null);
  assert.deepEqual(organiserHouseholds.data, [{ id: created.householdId }]);

  const outsiderEmail = `alex.${crypto.randomUUID()}@example.com`;
  const outsiderPassword = "another-correct-password-7";
  const outsiderClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await outsiderClient.auth.signUp({
    email: outsiderEmail,
    password: outsiderPassword,
    options: { data: { organiser_name: "Alex Morgan" } },
  })).error, null);
  const outsiderCode = await readLatestEmailCode(outsiderEmail);
  assert.equal((await outsiderClient.auth.verifyOtp({
    email: outsiderEmail,
    token: outsiderCode,
    type: "email",
  })).error, null);
  const outsiderHouseholds = await outsiderClient.from("households").select("id");
  assert.equal(outsiderHouseholds.error, null);
  assert.deepEqual(outsiderHouseholds.data, []);

  await accountService.signOut();
  assert.deepEqual(
    await accountService.register({ organiserName: "Someone Else", email, password }),
    { email },
  );
  await accountService.requestPasswordReset(`missing.${crypto.randomUUID()}@example.com`);
});

function memorySessionStorage() {
  const entries = new Map<string, string>();
  return {
    getItem(key: string) {
      return entries.get(key) ?? null;
    },
    setItem(key: string, value: string) {
      entries.set(key, value);
    },
    removeItem(key: string) {
      entries.delete(key);
    },
  };
}

async function readLatestEmailCode(email: string): Promise<string> {
  const response = await fetch(`http://127.0.0.1:54324/api/v1/search?query=to:${encodeURIComponent(email)}`);
  assert.equal(response.ok, true, "local verification email should be available");
  const search = await response.json() as { messages?: Array<{ ID: string }> };
  const messageId = search.messages?.[0]?.ID;
  assert.ok(messageId, `verification email for ${email} was not found`);

  const messageResponse = await fetch(`http://127.0.0.1:54324/api/v1/message/${messageId}`);
  assert.equal(messageResponse.ok, true, "local verification email should be readable");
  const message = await messageResponse.json() as { Text?: string; HTML?: string };
  const code = `${message.Text ?? ""} ${message.HTML ?? ""}`.match(/\b\d{6}\b/)?.[0];
  assert.ok(code, "verification email should contain a six-digit code");
  return code;
}

function readLocalSupabaseConfig(): { API_URL?: string; PUBLISHABLE_KEY?: string } {
  if (process.env.SUPABASE_URL && process.env.SUPABASE_PUBLISHABLE_KEY) return {};

  const command = path.resolve("node_modules", "supabase", "dist", "supabase.js");
  const status = spawnSync(
    process.execPath,
    [command, "status", "--output", "json", "--workdir", path.resolve("..")],
    { encoding: "utf8" },
  );
  assert.equal(status.status, 0, status.stderr || "local Supabase must be running");
  return JSON.parse(status.stdout) as { API_URL?: string; PUBLISHABLE_KEY?: string };
}

function createRecoveryAuthority(): { verificationKey: string; authorize: (message: string) => string } {
  const seed = randomBytes(32);
  const privateKey = createPrivateKey({
    key: Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), seed]),
    format: "der",
    type: "pkcs8",
  });
  const publicKey = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  return {
    verificationKey: publicKey.subarray(-32).toString("base64"),
    authorize: (message) => sign(null, Buffer.from(message, "utf8"), privateKey).toString("base64"),
  };
}

function recoverDeviceAuthorization(
  householdId: string,
  keyEpoch: number,
  devicePublicKey: string,
  keyEnvelope: string,
): string {
  return canonicalAuthorization("luna:recover-device:v2:", [
    householdId,
    keyEpoch.toString(),
    devicePublicKey,
    keyEnvelope,
  ]);
}

function revokeDeviceAuthorization(
  householdId: string,
  keyEpoch: number,
  revokedDeviceId: string,
  currentDevicePublicKey: string,
  recoveryEnvelope: string,
  deviceEnvelopes: Array<{ devicePublicKey: string; keyEnvelope: string }>,
): string {
  const sortedEnvelopes = [...deviceEnvelopes].sort((left, right) => (
    left.devicePublicKey.localeCompare(right.devicePublicKey)
  ));
  return canonicalAuthorization("luna:revoke-device:v2:", [
    householdId,
    keyEpoch.toString(),
    revokedDeviceId,
    currentDevicePublicKey,
    recoveryEnvelope,
    sortedEnvelopes.length.toString(),
    ...sortedEnvelopes.flatMap(({ devicePublicKey, keyEnvelope }) => [devicePublicKey, keyEnvelope]),
  ]);
}

function canonicalAuthorization(domainSeparator: string, fields: string[]): string {
  return domainSeparator + fields.map((field) => `${Buffer.byteLength(field, "utf8")}:${field}`).join("");
}
