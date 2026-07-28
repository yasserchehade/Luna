type PendingRevocation = { householdId: string; deviceId: string; alias: string };

export async function reconcileManagedIntelligenceAccess(dependencies: {
  listPendingRevocations(): Promise<PendingRevocation[]>;
  revokeGatewayAccessByAlias(alias: string): Promise<void>;
  recordGatewayRevoked(input: Pick<PendingRevocation, "householdId" | "deviceId">): Promise<void>;
}): Promise<{ revoked: number; failed: number }> {
  let revoked = 0;
  let failed = 0;
  for (const access of await dependencies.listPendingRevocations()) {
    try {
      await dependencies.revokeGatewayAccessByAlias(access.alias);
      await dependencies.recordGatewayRevoked(access);
      revoked += 1;
    } catch {
      failed += 1;
    }
  }
  return { revoked, failed };
}
