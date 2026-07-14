import type { AccountService, HouseholdSession } from "../account/accountService";
import type { TrustedDeviceService } from "./trustedDeviceService";

export async function synchronizeTrustedDevice(
  accountService: AccountService,
  trustedDeviceService: TrustedDeviceService,
  session: HouseholdSession,
): Promise<"active" | "revoked"> {
  const currentPublicKey = await trustedDeviceService.currentDevicePublicKey(session.householdId);
  const coordination = await accountService.getTrustedDeviceKeyCoordination(currentPublicKey);
  if (coordination.status !== "active") return "revoked";

  const localKeyEpoch = await trustedDeviceService.currentKeyEpoch(session.householdId);
  if (coordination.keyEpoch < localKeyEpoch) {
    throw new Error("The service returned a stale Household key epoch.");
  }
  if (
    coordination.keyEpoch > localKeyEpoch
    && await trustedDeviceService.isCurrentDeviceUnlocked(session.householdId)
  ) {
    await trustedDeviceService.applyRotatedDeviceEnvelope(
      session.householdId,
      coordination.keyEnvelope,
      coordination.keyEpoch,
    );
  }
  return "active";
}
