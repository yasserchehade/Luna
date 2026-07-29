import type { AccountService, HouseholdSession } from "./accountService";
import type { ConversationService } from "../conversation/conversationService";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

export type PostTrustSynchronizationResult = {
  portableMemory: "synchronized" | "failed";
  managedAccess: "synchronized" | "failed";
};

export async function settlePostTrustSynchronization(tasks: {
  portableMemory(): Promise<unknown>;
  managedAccess(): Promise<unknown>;
}): Promise<PostTrustSynchronizationResult> {
  const [portableMemory, managedAccess] = await Promise.allSettled([
    tasks.portableMemory(),
    tasks.managedAccess(),
  ]);
  return {
    portableMemory: portableMemory.status === "fulfilled" ? "synchronized" : "failed",
    managedAccess: managedAccess.status === "fulfilled" ? "synchronized" : "failed",
  };
}

export function postTrustSynchronizationNotice(
  result: PostTrustSynchronizationResult,
): string {
  if (result.portableMemory === "synchronized" && result.managedAccess === "synchronized") {
    return "";
  }
  if (result.portableMemory === "failed" && result.managedAccess === "synchronized") {
    return "Some protected Household memory could not be refreshed. Luna will retry it without blocking Cloud Assistance.";
  }
  if (result.portableMemory === "synchronized" && result.managedAccess === "failed") {
    return "Luna could not prepare Cloud Assistance on this Trusted Device. Local work remains available and Luna will retry automatically.";
  }
  return "Luna could not refresh some protected Household memory or prepare Cloud Assistance. Local work remains available and Luna will retry automatically.";
}

export async function synchronizeManagedIntelligenceAccess(
  accountService: AccountService,
  conversationService: ConversationService,
  trustedDeviceService: TrustedDeviceService,
  session: HouseholdSession,
  now: Date = new Date(),
): Promise<void> {
  const devicePublicKey = await trustedDeviceService.currentDevicePublicKey(session.householdId);
  const access = await accountService.getHouseholdIntelligenceAccess(devicePublicKey);
  if (access.entitlementState !== "entitled" || access.deviceState === "revoked") {
    await conversationService.clearManagedIntelligenceGatewayCredential(session.householdId);
    return;
  }

  const managedProvider = (await conversationService.listIntelligenceProviderStatuses(
    session.householdId,
  )).find(({ descriptor }) => descriptor.managedByLuna);
  const renewalThreshold = now.getTime() + 60 * 60 * 1_000;
  const credentialIsCurrent = access.deviceState === "ready"
    && access.credentialExpiresAt !== null
    && Date.parse(access.credentialExpiresAt) > renewalThreshold;
  if (managedProvider?.configured && credentialIsCurrent) return;

  const challenge = await accountService.beginManagedIntelligenceDeviceProvisioning(devicePublicKey);
  const authorizationSignature = await trustedDeviceService.signManagedIntelligenceDeviceProvisioning(
    session.householdId,
    challenge.nonce,
  );
  const provisioned = await accountService.provisionManagedIntelligenceDeviceAccess({
    devicePublicKey,
    challengeId: challenge.id,
    nonce: challenge.nonce,
    authorizationSignature,
  });
  await conversationService.setManagedIntelligenceGatewayCredential(
    session.householdId,
    provisioned.credential,
  );
}
