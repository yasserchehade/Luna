type DeviceProof = {
  devicePublicKey: string;
  challengeId: string;
  nonce: string;
  authorizationSignature: string;
};

type ProvisioningDependencies = {
  authorizeDevice(
    request: Request,
    proof: DeviceProof,
  ): Promise<{
    householdId: string;
    deviceId: string;
    existingAlias: string | null;
    budgetScopeId: string;
    maxBudgetUsd: number;
  } | null>;
  createGatewayAccess(input: {
    householdId: string;
    deviceId: string;
    alias: string;
    budgetScopeId: string;
    maxBudgetUsd: number;
  }): Promise<{ alias: string; credential: string; expiresAt: string }>;
  reserveGatewayAlias(input: {
    householdId: string;
    deviceId: string;
    alias: string;
    budgetScopeId: string;
    maxBudgetUsd: number;
  }): Promise<void>;
  recordReady(input: {
    householdId: string;
    deviceId: string;
    alias: string;
    expiresAt: string;
  }): Promise<void>;
  revokeGatewayAccess?(credential: string): Promise<void>;
  revokeGatewayAccessByAlias?(alias: string): Promise<void>;
};

export async function handleManagedIntelligenceProvisioning(
  request: Request,
  dependencies: ProvisioningDependencies,
): Promise<Response> {
  if (request.method !== "POST") return json({ error: "Method not allowed" }, 405);
  let proof: DeviceProof;
  try {
    const input = await request.json() as Partial<DeviceProof>;
    if (![input.devicePublicKey, input.challengeId, input.nonce, input.authorizationSignature]
      .every((value) => typeof value === "string" && value.trim() !== "")) {
      return json({ error: "Trusted Device proof is required" }, 400);
    }
    proof = input as DeviceProof;
  } catch {
    return json({ error: "Invalid request" }, 400);
  }

  const authorization = await dependencies.authorizeDevice(request, proof);
  if (!authorization) return json({ error: "Trusted Device authorization is required" }, 401);
  if (authorization.existingAlias) {
    await dependencies.revokeGatewayAccessByAlias?.(authorization.existingAlias);
  }
  const reservedAlias = `luna-managed-${authorization.deviceId}`;
  await dependencies.reserveGatewayAlias({
    householdId: authorization.householdId,
    deviceId: authorization.deviceId,
    alias: reservedAlias,
    budgetScopeId: authorization.budgetScopeId,
    maxBudgetUsd: authorization.maxBudgetUsd,
  });
  const access = await dependencies.createGatewayAccess({
    householdId: authorization.householdId,
    deviceId: authorization.deviceId,
    alias: reservedAlias,
    budgetScopeId: authorization.budgetScopeId,
    maxBudgetUsd: authorization.maxBudgetUsd,
  });
  if (access.alias !== reservedAlias || !access.credential || Number.isNaN(Date.parse(access.expiresAt))) {
    return json({ error: "Managed access could not be provisioned" }, 502);
  }
  try {
    await dependencies.recordReady({
      householdId: authorization.householdId,
      deviceId: authorization.deviceId,
      alias: access.alias,
      expiresAt: access.expiresAt,
    });
  } catch (error) {
    await dependencies.revokeGatewayAccess?.(access.credential).catch(() => undefined);
    throw error;
  }
  return json({ state: "ready", credential: access.credential, expiresAt: access.expiresAt }, 200);
}

function json(value: Record<string, unknown>, status: number): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
}
