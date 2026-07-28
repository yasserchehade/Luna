import type { AccountService, HouseholdSession } from "./accountService";
import type { ConversationService } from "../conversation/conversationService";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

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
