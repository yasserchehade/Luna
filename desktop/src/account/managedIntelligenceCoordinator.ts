import type { AccountService, HouseholdSession } from "./accountService";
import type { ConversationService } from "../conversation/conversationService";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

export async function synchronizeManagedIntelligenceAccess(
  accountService: AccountService,
  conversationService: ConversationService,
  trustedDeviceService: TrustedDeviceService,
  session: HouseholdSession,
): Promise<void> {
  const access = await accountService.getHouseholdIntelligenceAccess();
  if (!["provisioning", "ready"].includes(access.state)) {
    await conversationService.clearManagedIntelligenceGatewayCredential(session.householdId);
    return;
  }

  const managedProvider = (await conversationService.listIntelligenceProviderStatuses(
    session.householdId,
  )).find(({ descriptor }) => descriptor.managedByLuna);
  if (managedProvider?.configured) return;

  const devicePublicKey = await trustedDeviceService.currentDevicePublicKey(session.householdId);
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
