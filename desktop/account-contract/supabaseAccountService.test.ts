import assert from "node:assert/strict";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { createHmac, createPrivateKey, createPublicKey, randomBytes, sign } from "node:crypto";
import test from "node:test";
import { createClient } from "@supabase/supabase-js";
import * as OTPAuth from "otpauth";
import postgres from "postgres";
import { SupabaseAccountService } from "../src/account/supabaseAccountService";

const localConfig = readLocalSupabaseConfig();
const supabaseUrl = process.env.SUPABASE_URL ?? localConfig.API_URL;
const publishableKey = process.env.SUPABASE_PUBLISHABLE_KEY ?? localConfig.PUBLISHABLE_KEY;
const serviceRoleKey = process.env.SUPABASE_SERVICE_ROLE_KEY ?? localConfig.SERVICE_ROLE_KEY;
const jwtSecret = process.env.SUPABASE_JWT_SECRET ?? localConfig.JWT_SECRET;
const databaseUrl = process.env.SUPABASE_DB_URL ?? localConfig.DB_URL;

function futureIso(days: number): string {
  const value = new Date(Date.now() + days * 24 * 60 * 60 * 1_000);
  value.setUTCSeconds(0, 0);
  return value.toISOString();
}

function postgresIso(value: string): string {
  return value.replace(".000Z", "+00:00");
}

test("complimentary Managed Intelligence is granted by an operator at the Household boundary", async () => {
  assert.ok(supabaseUrl, "SUPABASE_URL is required");
  assert.ok(publishableKey, "SUPABASE_PUBLISHABLE_KEY is required");
  assert.ok(serviceRoleKey, "SUPABASE_SERVICE_ROLE_KEY is required");

  const email = `entitlement.${crypto.randomUUID()}@example.com`;
  const password = "correct-horse-battery-staple-7";
  const validUntil = futureIso(35);
  const accountService = new SupabaseAccountService(supabaseUrl, publishableKey);
  await accountService.register({ organiserName: "Alex Morgan", email, password });
  await accountService.verifyEmail(email, await readLatestEmailCode(email));
  const household = await accountService.createHousehold("Morgan Household");

  assert.deepEqual(await accountService.getHouseholdIntelligenceAccess(), {
    householdId: household.householdId,
    plan: "free",
    entitlementState: "free",
    deviceState: "notApplicable",
    entitlementSource: null,
    maxBudgetUsd: null,
    validUntil: null,
    credentialExpiresAt: null,
  });

  const memberClient = createClient(supabaseUrl, publishableKey, {
    auth: { persistSession: false },
  });
  assert.equal((await memberClient.auth.signInWithPassword({ email, password })).error, null);
  const forbiddenGrant = await memberClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 1,
    requested_valid_until: validUntil,
  });
  assert.ok(forbiddenGrant.error, "a Household Member must not grant its own entitlement");

  const operatorClient = createClient(supabaseUrl, serviceRoleKey, {
    auth: { persistSession: false },
  });
  const grant = await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 1,
    requested_valid_until: validUntil,
  });
  assert.equal(grant.error, null);

  assert.deepEqual(await accountService.getHouseholdIntelligenceAccess(), {
    householdId: household.householdId,
    plan: "managed",
    entitlementState: "entitled",
    deviceState: "pending",
    entitlementSource: "complimentary",
    maxBudgetUsd: 1,
    validUntil: postgresIso(validUntil),
    credentialExpiresAt: null,
  });
});

test("verified Paddle events are idempotent and older subscription state cannot win", async () => {
  assert.ok(supabaseUrl, "SUPABASE_URL is required");
  assert.ok(publishableKey, "SUPABASE_PUBLISHABLE_KEY is required");
  assert.ok(serviceRoleKey, "SUPABASE_SERVICE_ROLE_KEY is required");

  const email = `billing.${crypto.randomUUID()}@example.com`;
  const password = "correct-horse-battery-staple-7";
  const occurredAt = futureIso(0);
  const staleOccurredAt = new Date(Date.parse(occurredAt) - 3 * 60 * 1_000).toISOString();
  const paymentProblemOccurredAt = new Date(Date.parse(occurredAt) + 3 * 60 * 1_000).toISOString();
  const billingValidUntil = futureIso(31);
  const complimentaryValidUntil = futureIso(35);
  const accountService = new SupabaseAccountService(supabaseUrl, publishableKey);
  await accountService.register({ organiserName: "Taylor Morgan", email, password });
  await accountService.verifyEmail(email, await readLatestEmailCode(email));
  const household = await accountService.createHousehold("Taylor Household");
  const operatorClient = createClient(supabaseUrl, serviceRoleKey, {
    auth: { persistSession: false },
  });
  const memberClient = createClient(supabaseUrl, publishableKey, {
    auth: { persistSession: false },
  });
  assert.equal((await memberClient.auth.signInWithPassword({ email, password })).error, null);
  const billingContext = await memberClient.rpc("current_household_billing_context");
  assert.equal(billingContext.error, null);
  assert.deepEqual(Array.isArray(billingContext.data) ? billingContext.data[0] : billingContext.data, {
    household_id: household.householdId,
    organiser_email: email,
    external_customer_id: null,
    external_subscription_id: null,
  });
  const billingRunId = crypto.randomUUID();
  const checkout = await operatorClient.rpc("record_paddle_checkout_pending", {
    requested_household_id: household.householdId,
    requested_transaction_id: `txn_${billingRunId}`,
  });
  assert.equal(checkout.error, null);
  assert.equal((await accountService.getHouseholdIntelligenceAccess()).entitlementState, "checkoutPending");
  const activeEvent = {
    requested_event_id: `evt_active_${billingRunId}`,
    requested_event_type: "subscription.updated",
    requested_occurred_at: occurredAt,
    requested_household_id: household.householdId,
    requested_customer_id: `ctm_${billingRunId}`,
    requested_subscription_id: `sub_${billingRunId}`,
    requested_status: "active",
    requested_valid_until: billingValidUntil,
    requested_max_budget_usd: 2,
  };

  const first = await operatorClient.rpc("apply_paddle_subscription_event", activeEvent);
  assert.equal(first.error, null);
  assert.equal(first.data, true);
  assert.equal((await operatorClient.rpc("apply_paddle_subscription_event", activeEvent)).data, false);
  assert.deepEqual(await accountService.getHouseholdIntelligenceAccess(), {
    householdId: household.householdId,
    plan: "managed",
    entitlementState: "entitled",
    deviceState: "pending",
    entitlementSource: "billing",
    maxBudgetUsd: 2,
    validUntil: postgresIso(billingValidUntil),
    credentialExpiresAt: null,
  });

  const paymentProblem = await operatorClient.rpc("apply_paddle_subscription_event", {
    ...activeEvent,
    requested_event_id: `evt_past_due_${billingRunId}`,
    requested_occurred_at: paymentProblemOccurredAt,
    requested_status: "past_due",
  });
  assert.equal(paymentProblem.error, null);
  assert.equal(paymentProblem.data, true);

  const staleActive = await operatorClient.rpc("apply_paddle_subscription_event", {
    ...activeEvent,
    requested_event_id: `evt_stale_active_${billingRunId}`,
    requested_occurred_at: staleOccurredAt,
  });
  assert.equal(staleActive.error, null);
  assert.equal(staleActive.data, false);
  assert.equal((await accountService.getHouseholdIntelligenceAccess()).entitlementState, "paymentProblem");

  assert.equal((await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 1,
    requested_valid_until: complimentaryValidUntil,
  })).error, null);
  assert.deepEqual(await accountService.getHouseholdIntelligenceAccess(), {
    householdId: household.householdId,
    plan: "managed",
    entitlementState: "entitled",
    deviceState: "pending",
    entitlementSource: "complimentary",
    maxBudgetUsd: 1,
    validUntil: postgresIso(complimentaryValidUntil),
    credentialExpiresAt: null,
  });
});

test("an entitled Trusted Device proves possession before managed access is provisioned", async () => {
  assert.ok(supabaseUrl, "SUPABASE_URL is required");
  assert.ok(publishableKey, "SUPABASE_PUBLISHABLE_KEY is required");
  assert.ok(serviceRoleKey, "SUPABASE_SERVICE_ROLE_KEY is required");
  assert.ok(databaseUrl, "SUPABASE_DB_URL is required");

  const email = `provisioning.${crypto.randomUUID()}@example.com`;
  const password = "correct-horse-battery-staple-7";
  const validUntil = futureIso(35);
  const credentialExpiresAt = futureIso(1);
  const replacementValidUntil = futureIso(65);
  const accountService = new SupabaseAccountService(supabaseUrl, publishableKey);
  await accountService.register({ organiserName: "Jordan Lee", email, password });
  await accountService.verifyEmail(email, await readLatestEmailCode(email));
  const household = await accountService.createHousehold("Lee Household");
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
  const deviceAuthority = createSigningAuthority();
  const recoveryAuthority = createSigningAuthority();
  const device = await accountService.registerFirstTrustedDevice({
    label: "Jordan's PC",
    publicKey: `age1${crypto.randomUUID()}`,
    authorizationPublicKey: deviceAuthority.verificationKey,
    keyEnvelope: "encrypted-device-envelope",
    recoveryEnvelope: "encrypted-recovery-envelope",
    recoveryVerificationKey: recoveryAuthority.verificationKey,
  });
  const operatorClient = createClient(supabaseUrl, serviceRoleKey, {
    auth: { persistSession: false },
  });
  assert.equal((await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 1,
    requested_valid_until: validUntil,
  })).error, null);

  const challenge = await accountService.beginManagedIntelligenceDeviceProvisioning(device.publicKey);
  const authorization = deviceAuthority.authorize(canonicalAuthorization(
    "luna:managed-intelligence-device:v1:",
    [household.householdId, device.publicKey, challenge.nonce],
  ));
  const memberClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await memberClient.auth.signInWithPassword({ email, password })).error, null);
  const factors = await memberClient.auth.mfa.listFactors();
  assert.equal(factors.error, null);
  const memberFactor = factors.data?.totp.find(({ status }) => status === "verified");
  assert.ok(memberFactor);
  const memberChallenge = await memberClient.auth.mfa.challenge({ factorId: memberFactor.id });
  assert.equal(memberChallenge.error, null);
  const memberVerification = await memberClient.auth.mfa.verify({
    factorId: memberFactor.id,
    challengeId: memberChallenge.data.id,
    code: totp.generate(),
  });
  assert.equal(memberVerification.error, null);
  const authorized = await memberClient.rpc("authorize_managed_intelligence_device_provisioning", {
    requested_device_public_key: device.publicKey,
    requested_challenge_id: challenge.id,
    requested_nonce: challenge.nonce,
    requested_authorization_signature: authorization,
  });
  assert.equal(authorized.error, null);
  const authorizedRow = Array.isArray(authorized.data) ? authorized.data[0] : authorized.data;
  assert.equal(authorizedRow.household_id, household.householdId);
  assert.equal(authorizedRow.device_id, device.id);
  assert.equal(authorizedRow.existing_gateway_key_alias, null);
  assert.equal(Number(authorizedRow.max_budget_usd), 1);
  assert.equal((await operatorClient.rpc("reserve_managed_intelligence_device_gateway_alias", {
    requested_household_id: household.householdId,
    requested_device_id: device.id,
    requested_gateway_key_alias: `luna-managed-${device.id}`,
    requested_budget_scope_id: authorizedRow.budget_scope_id,
    requested_max_budget_usd: Number(authorizedRow.max_budget_usd),
  })).error, null);
  assert.equal((await operatorClient.rpc("record_managed_intelligence_device_access", {
    requested_household_id: household.householdId,
    requested_device_id: device.id,
    requested_status: "ready",
    requested_gateway_key_alias: `luna-managed-${device.id}`,
    requested_credential_expires_at: credentialExpiresAt,
  })).error, null);
  const ready = await accountService.getHouseholdIntelligenceAccess(device.publicKey);
  assert.equal(ready.entitlementState, "entitled");
  assert.equal(ready.deviceState, "ready");
  assert.equal(ready.credentialExpiresAt, postgresIso(credentialExpiresAt));
  assert.ok((await memberClient.rpc("authorize_managed_intelligence_device_provisioning", {
    requested_device_public_key: device.publicKey,
    requested_challenge_id: challenge.id,
    requested_nonce: challenge.nonce,
    requested_authorization_signature: authorization,
  })).error, "a provisioning challenge cannot be replayed");
  assert.equal((await operatorClient.rpc("revoke_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
  })).error, null);
  assert.equal((await accountService.getHouseholdIntelligenceAccess(device.publicKey)).entitlementState, "ended");
  assert.equal((await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 0.5,
    requested_valid_until: replacementValidUntil,
  })).error, null);
  await assert.rejects(
    () => accountService.beginManagedIntelligenceDeviceProvisioning(device.publicKey),
  );
  const pendingRevocations = await operatorClient.rpc("pending_managed_intelligence_revocations");
  assert.equal(pendingRevocations.error, null);
  assert.deepEqual(pendingRevocations.data.filter(
    (row: { household_id: string }) => row.household_id === household.householdId,
  ), [{
    household_id: household.householdId,
    device_id: device.id,
    gateway_key_alias: `luna-managed-${device.id}`,
  }]);
  assert.equal((await operatorClient.rpc("record_managed_intelligence_gateway_revoked", {
    requested_household_id: household.householdId,
    requested_device_id: device.id,
  })).error, null);
  assert.deepEqual(
    (await operatorClient.rpc("pending_managed_intelligence_revocations")).data
      .filter((row: { household_id: string }) => row.household_id === household.householdId),
    [],
  );
  const replacementChallenge = await accountService.beginManagedIntelligenceDeviceProvisioning(
    device.publicKey,
  );
  const replacementAuthorization = deviceAuthority.authorize(canonicalAuthorization(
    "luna:managed-intelligence-device:v1:",
    [household.householdId, device.publicKey, replacementChallenge.nonce],
  ));
  const replacementAuthorized = await memberClient.rpc("authorize_managed_intelligence_device_provisioning", {
    requested_device_public_key: device.publicKey,
    requested_challenge_id: replacementChallenge.id,
    requested_nonce: replacementChallenge.nonce,
    requested_authorization_signature: replacementAuthorization,
  });
  assert.equal(replacementAuthorized.error, null);
  const replacementAuthorizedRow = Array.isArray(replacementAuthorized.data)
    ? replacementAuthorized.data[0]
    : replacementAuthorized.data;
  assert.equal((await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 0.25,
    requested_valid_until: replacementValidUntil,
  })).error, null);
  assert.ok((await operatorClient.rpc("reserve_managed_intelligence_device_gateway_alias", {
    requested_household_id: household.householdId,
    requested_device_id: device.id,
    requested_gateway_key_alias: `luna-managed-${device.id}`,
    requested_budget_scope_id: replacementAuthorizedRow.budget_scope_id,
    requested_max_budget_usd: Number(replacementAuthorizedRow.max_budget_usd),
  })).error, "a stale budget authorization cannot restore an earlier Household cap");

  assert.equal((await operatorClient.rpc("grant_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
    requested_max_budget_usd: 0.25,
    requested_valid_until: replacementValidUntil,
  })).error, null);
  const raceChallenge = await accountService.beginManagedIntelligenceDeviceProvisioning(device.publicKey);
  const raceAuthorization = deviceAuthority.authorize(canonicalAuthorization(
    "luna:managed-intelligence-device:v1:",
    [household.householdId, device.publicKey, raceChallenge.nonce],
  ));
  const raceAuthorized = await memberClient.rpc("authorize_managed_intelligence_device_provisioning", {
    requested_device_public_key: device.publicKey,
    requested_challenge_id: raceChallenge.id,
    requested_nonce: raceChallenge.nonce,
    requested_authorization_signature: raceAuthorization,
  });
  assert.equal(raceAuthorized.error, null);
  const raceAuthorizedRow = Array.isArray(raceAuthorized.data) ? raceAuthorized.data[0] : raceAuthorized.data;
  assert.equal((await operatorClient.rpc("reserve_managed_intelligence_device_gateway_alias", {
    requested_household_id: household.householdId,
    requested_device_id: device.id,
    requested_gateway_key_alias: `luna-managed-${device.id}`,
    requested_budget_scope_id: raceAuthorizedRow.budget_scope_id,
    requested_max_budget_usd: Number(raceAuthorizedRow.max_budget_usd),
  })).error, null);
  assert.equal((await operatorClient.rpc("revoke_complimentary_managed_intelligence", {
    requested_household_id: household.householdId,
  })).error, null);
  const leasedRevocations = await operatorClient.rpc("pending_managed_intelligence_revocations");
  assert.equal(leasedRevocations.error, null);
  assert.deepEqual(leasedRevocations.data.filter(
    (row: { household_id: string }) => row.household_id === household.householdId,
  ), [], "reconciliation cannot finish before an in-flight mint has left its alias usable");
  const database = postgres(databaseUrl, { max: 1 });
  await database`
    update public.managed_intelligence_device_access
    set provisioning_lease_expires_at = now() - interval '1 second'
    where household_id = ${household.householdId} and device_id = ${device.id}
  `;
  await database.end();
  const raceRevocations = await operatorClient.rpc("pending_managed_intelligence_revocations");
  assert.equal(raceRevocations.error, null);
  assert.deepEqual(raceRevocations.data.filter(
    (row: { household_id: string }) => row.household_id === household.householdId,
  ), [{
    household_id: household.householdId,
    device_id: device.id,
    gateway_key_alias: `luna-managed-${device.id}`,
  }], "a reserved alias remains durable if entitlement is revoked before readiness is recorded");
});

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
  const recoveryAuthority = createSigningAuthority();
  const firstDeviceAuthority = createSigningAuthority();

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
    authorizationPublicKey: firstDeviceAuthority.verificationKey,
    keyEnvelope: "encrypted-device-envelope",
    recoveryEnvelope: "encrypted-recovery-envelope",
    recoveryVerificationKey: recoveryAuthority.verificationKey,
  });
  assert.equal(trustedDevice.label, "Sam's test PC");
  assert.equal(trustedDevice.status, "active");
  assert.deepEqual(await accountService.getTrustedDeviceRecoveryEnvelope(), {
    recoveryEnvelope: "encrypted-recovery-envelope",
    recoveryVerificationKey: recoveryAuthority.verificationKey,
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
  const recoveredDeviceAuthority = createSigningAuthority();
  const recoveredAuthorization = recoveryAuthority.authorize(recoverDeviceAuthorization(
    created.householdId,
    1,
    recoveredPublicKey,
    recoveredDeviceAuthority.verificationKey,
    recoveredKeyEnvelope,
  ));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Unauthorised replacement",
    publicKey: recoveredPublicKey,
    authorizationPublicKey: recoveredDeviceAuthority.verificationKey,
    keyEnvelope: recoveredKeyEnvelope,
    keyEpoch: 1,
    recoveryAuthorizationSignature: randomBytes(64).toString("base64"),
  }));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Missing recovery authority",
    publicKey: recoveredPublicKey,
    authorizationPublicKey: recoveredDeviceAuthority.verificationKey,
    keyEnvelope: recoveredKeyEnvelope,
    keyEpoch: 1,
    recoveryAuthorizationSignature: null as unknown as string,
  }));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Substituted replacement envelope",
    publicKey: recoveredPublicKey,
    authorizationPublicKey: recoveredDeviceAuthority.verificationKey,
    keyEnvelope: "substituted-replacement-device-envelope",
    keyEpoch: 1,
    recoveryAuthorizationSignature: recoveredAuthorization,
  }));
  const recoveredDevice = await accountService.registerRecoveredTrustedDevice({
    label: "Sam's replacement PC",
    publicKey: recoveredPublicKey,
    authorizationPublicKey: recoveredDeviceAuthority.verificationKey,
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
    recoveryVerificationKey: recoveryAuthority.verificationKey,
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

  const replacementRecoveryAuthority = createSigningAuthority();
  const replacementRecoveryEnvelope = "replacement-recovery-envelope";
  const replacementAuthorization = recoveredDeviceAuthority.authorize(replaceRecoveryKeyAuthorization(
    created.householdId,
    2,
    recoveredDevice.publicKey,
    recoveryAuthority.verificationKey,
    replacementRecoveryEnvelope,
    replacementRecoveryAuthority.verificationKey,
  ));
  assert.ok(jwtSecret, "SUPABASE_JWT_SECRET is required for the stale-MFA boundary test");
  const staleIdentityClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  const staleIdentity = await staleIdentityClient.auth.signInWithPassword({ email, password: replacementPassword });
  assert.equal(staleIdentity.error, null);
  assert.ok(staleIdentity.data.user);
  const staleMfaToken = createTestJwt(jwtSecret, {
    sub: staleIdentity.data.user.id,
    email,
    role: "authenticated",
    aud: "authenticated",
    aal: "aal2",
    amr: [{ method: "totp", timestamp: Math.floor(Date.now() / 1000) - 10 * 60 }],
  });
  const staleMfaClient = createClient(supabaseUrl, publishableKey, {
    auth: { persistSession: false },
    global: { headers: { Authorization: `Bearer ${staleMfaToken}` } },
  });
  const staleMfaReplacement = await staleMfaClient.rpc("replace_recovery_key", {
    requested_current_device_public_key: recoveredDevice.publicKey,
    requested_current_key_epoch: 2,
    requested_current_recovery_verification_key: recoveryAuthority.verificationKey,
    requested_recovery_envelope: replacementRecoveryEnvelope,
    requested_recovery_verification_key: replacementRecoveryAuthority.verificationKey,
    requested_device_authorization_signature: replacementAuthorization,
  });
  assert.match(
    staleMfaReplacement.error?.message ?? "",
    /Fresh authenticator verification is required/,
    "a stale AAL2 session must not authorize Recovery Key Replacement",
  );
  await assert.rejects(accountService.replaceRecoveryKey({
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 2,
    currentRecoveryVerificationKey: recoveryAuthority.verificationKey,
    recoveryEnvelope: replacementRecoveryEnvelope,
    recoveryVerificationKey: replacementRecoveryAuthority.verificationKey,
    deviceAuthorizationSignature: randomBytes(64).toString("base64"),
  }));
  await assert.rejects(accountService.replaceRecoveryKey({
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 2,
    currentRecoveryVerificationKey: recoveryAuthority.verificationKey,
    recoveryEnvelope: "substituted-recovery-envelope",
    recoveryVerificationKey: replacementRecoveryAuthority.verificationKey,
    deviceAuthorizationSignature: replacementAuthorization,
  }));
  const concurrentDeviceAuthority = createSigningAuthority();
  const concurrentDevicePublicKey = "age1concurrentoldrecoverydevice";
  const concurrentDeviceEnvelope = "concurrent-old-recovery-device-envelope";
  const concurrentRecoveryAuthorization = recoveryAuthority.authorize(recoverDeviceAuthorization(
    created.householdId,
    2,
    concurrentDevicePublicKey,
    concurrentDeviceAuthority.verificationKey,
    concurrentDeviceEnvelope,
  ));
  const [replacementResult, staleEnrollmentResult] = await whileRecoveryAuthorityRowIsLocked(
    created.householdId,
    async () => {
      const replacement = accountService.replaceRecoveryKey({
        currentDevicePublicKey: recoveredDevice.publicKey,
        currentKeyEpoch: 2,
        currentRecoveryVerificationKey: recoveryAuthority.verificationKey,
        recoveryEnvelope: replacementRecoveryEnvelope,
        recoveryVerificationKey: replacementRecoveryAuthority.verificationKey,
        deviceAuthorizationSignature: replacementAuthorization,
      });
      await new Promise((resolve) => setTimeout(resolve, 100));
      const staleEnrollment = accountService.registerRecoveredTrustedDevice({
        label: "Concurrent device using previous Recovery Key",
        publicKey: concurrentDevicePublicKey,
        authorizationPublicKey: concurrentDeviceAuthority.verificationKey,
        keyEnvelope: concurrentDeviceEnvelope,
        keyEpoch: 2,
        recoveryAuthorizationSignature: concurrentRecoveryAuthorization,
      });
      return Promise.allSettled([replacement, staleEnrollment]);
    },
  );
  assert.equal(replacementResult.status, "fulfilled", "replacement should win the queued authority update");
  assert.equal(staleEnrollmentResult.status, "rejected", "old Recovery Key enrollment must not commit after replacement");
  assert.deepEqual(await accountService.getTrustedDeviceRecoveryEnvelope(), {
    recoveryEnvelope: replacementRecoveryEnvelope,
    recoveryVerificationKey: replacementRecoveryAuthority.verificationKey,
    keyEpoch: 2,
  });
  await assert.rejects(accountService.replaceRecoveryKey({
    currentDevicePublicKey: recoveredDevice.publicKey,
    currentKeyEpoch: 2,
    currentRecoveryVerificationKey: recoveryAuthority.verificationKey,
    recoveryEnvelope: replacementRecoveryEnvelope,
    recoveryVerificationKey: replacementRecoveryAuthority.verificationKey,
    deviceAuthorizationSignature: replacementAuthorization,
  }), "a committed Recovery Key Replacement cannot be replayed");

  const postReplacementDeviceAuthority = createSigningAuthority();
  const postReplacementPublicKey = "age1postreplacementdevicepublickey";
  const postReplacementEnvelope = "post-replacement-device-envelope";
  const oldRecoveryAuthorization = recoveryAuthority.authorize(recoverDeviceAuthorization(
    created.householdId,
    2,
    postReplacementPublicKey,
    postReplacementDeviceAuthority.verificationKey,
    postReplacementEnvelope,
  ));
  await assert.rejects(accountService.registerRecoveredTrustedDevice({
    label: "Device using previous Recovery Key",
    publicKey: postReplacementPublicKey,
    authorizationPublicKey: postReplacementDeviceAuthority.verificationKey,
    keyEnvelope: postReplacementEnvelope,
    keyEpoch: 2,
    recoveryAuthorizationSignature: oldRecoveryAuthorization,
  }));
  const newRecoveryAuthorization = replacementRecoveryAuthority.authorize(recoverDeviceAuthorization(
    created.householdId,
    2,
    postReplacementPublicKey,
    postReplacementDeviceAuthority.verificationKey,
    postReplacementEnvelope,
  ));
  const postReplacementDevice = await accountService.registerRecoveredTrustedDevice({
    label: "Device using replacement Recovery Key",
    publicKey: postReplacementPublicKey,
    authorizationPublicKey: postReplacementDeviceAuthority.verificationKey,
    keyEnvelope: postReplacementEnvelope,
    keyEpoch: 2,
    recoveryAuthorizationSignature: newRecoveryAuthorization,
  });
  assert.equal(postReplacementDevice.status, "active");

  const organiserClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await organiserClient.auth.signInWithPassword({ email, password: replacementPassword })).error, null);
  const aal1Replacement = await organiserClient.rpc("replace_recovery_key", {
    requested_current_device_public_key: recoveredDevice.publicKey,
    requested_current_key_epoch: 2,
    requested_current_recovery_verification_key: recoveryAuthority.verificationKey,
    requested_recovery_envelope: replacementRecoveryEnvelope,
    requested_recovery_verification_key: replacementRecoveryAuthority.verificationKey,
    requested_device_authorization_signature: replacementAuthorization,
  });
  assert.notEqual(aal1Replacement.error, null, "authenticator MFA must be required for replacement");
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

function readLocalSupabaseConfig(): {
  API_URL?: string;
  DB_URL?: string;
  JWT_SECRET?: string;
  PUBLISHABLE_KEY?: string;
  SERVICE_ROLE_KEY?: string;
} {
  if (process.env.SUPABASE_URL && process.env.SUPABASE_PUBLISHABLE_KEY) return {};

  const command = path.resolve("node_modules", "supabase", "dist", "supabase.js");
  const status = spawnSync(
    process.execPath,
    [command, "status", "--output", "json", "--workdir", path.resolve("..")],
    { encoding: "utf8" },
  );
  assert.equal(status.status, 0, status.stderr || "local Supabase must be running");
  return JSON.parse(status.stdout) as {
    API_URL?: string;
    DB_URL?: string;
    JWT_SECRET?: string;
    PUBLISHABLE_KEY?: string;
    SERVICE_ROLE_KEY?: string;
  };
}

function createSigningAuthority(): { verificationKey: string; authorize: (message: string) => string } {
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
  deviceAuthorizationPublicKey: string,
  keyEnvelope: string,
): string {
  return canonicalAuthorization("luna:recover-device:v2:", [
    householdId,
    keyEpoch.toString(),
    devicePublicKey,
    deviceAuthorizationPublicKey,
    keyEnvelope,
  ]);
}

function createTestJwt(secret: string, claims: Record<string, unknown>): string {
  const now = Math.floor(Date.now() / 1000);
  const encode = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  const unsigned = `${encode({ alg: "HS256", typ: "JWT" })}.${encode({
    ...claims,
    iat: now,
    exp: now + 5 * 60,
  })}`;
  const signature = createHmac("sha256", secret).update(unsigned).digest("base64url");
  return `${unsigned}.${signature}`;
}

async function whileRecoveryAuthorityRowIsLocked<T>(
  householdId: string,
  operation: () => Promise<T>,
): Promise<T> {
  assert.match(householdId, /^[0-9a-f-]{36}$/i);
  assert.ok(databaseUrl, "SUPABASE_DB_URL is required for the recovery-authority race test");
  const database = postgres(databaseUrl, { max: 1 });
  let result: Promise<T> | undefined;
  try {
    await database.begin(async (transaction) => {
      await transaction`
        select key_epoch
        from public.household_key_epochs
        where household_id = ${householdId}
        for update
      `;
      result = operation();
      await new Promise((resolve) => setTimeout(resolve, 300));
    });
    assert.ok(result, "the recovery-authority race operation did not start");
    return await result;
  } finally {
    await database.end();
  }
}

function replaceRecoveryKeyAuthorization(
  householdId: string,
  keyEpoch: number,
  currentDevicePublicKey: string,
  currentRecoveryVerificationKey: string,
  recoveryEnvelope: string,
  recoveryVerificationKey: string,
): string {
  return canonicalAuthorization("luna:replace-recovery-key:v1:", [
    householdId,
    keyEpoch.toString(),
    currentDevicePublicKey,
    currentRecoveryVerificationKey,
    recoveryEnvelope,
    recoveryVerificationKey,
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
