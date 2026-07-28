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
  ): Promise<{ householdId: string; deviceId: string } | null>;
  createGatewayAccess(input: {
    householdId: string;
    deviceId: string;
  }): Promise<{ alias: string; credential: string }>;
  recordReady(input: {
    householdId: string;
    deviceId: string;
    alias: string;
  }): Promise<void>;
  revokeGatewayAccess?(credential: string): Promise<void>;
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
  const access = await dependencies.createGatewayAccess(authorization);
  if (!access.alias || !access.credential) {
    return json({ error: "Managed access could not be provisioned" }, 502);
  }
  try {
    await dependencies.recordReady({ ...authorization, alias: access.alias });
  } catch (error) {
    await dependencies.revokeGatewayAccess?.(access.credential).catch(() => undefined);
    throw error;
  }
  return json({ state: "ready", credential: access.credential }, 200);
}

function json(value: Record<string, unknown>, status: number): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
}
