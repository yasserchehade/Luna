type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

export function createLiteLlmManagedAccessClient(configuration: {
  adminEndpoint: string;
  endpoint: string;
  masterKey: string;
  maxBudgetUsd: number;
  duration: string;
  fetch: FetchLike;
}) {
  const chatEndpoint = new URL(configuration.endpoint);
  const adminEndpoint = new URL(configuration.adminEndpoint);
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (chatEndpoint.protocol !== "https:" && !loopbackHosts.has(chatEndpoint.hostname)) {
    throw new Error("The managed gateway must use HTTPS unless it is loopback-only.");
  }
  if (!chatEndpoint.pathname.endsWith("/v1/chat/completions")) {
    throw new Error("The managed gateway endpoint is invalid.");
  }
  if (adminEndpoint.protocol !== "https:" && !loopbackHosts.has(adminEndpoint.hostname)) {
    throw new Error("The managed gateway administration endpoint must use HTTPS unless it is loopback-only.");
  }
  if (!Number.isFinite(configuration.maxBudgetUsd) || configuration.maxBudgetUsd <= 0) {
    throw new Error("The managed gateway budget is invalid.");
  }

  const request = async (path: string, body: Record<string, unknown>) => {
    const response = await configuration.fetch(new URL(path, adminEndpoint).toString(), {
      method: "POST",
      headers: {
        authorization: `Bearer ${configuration.masterKey}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error("Managed gateway provisioning is unavailable.");
    return response.json() as Promise<Record<string, unknown>>;
  };

  return {
    async createGatewayAccess(input: { householdId: string; deviceId: string }) {
      const alias = `luna-managed-${input.deviceId}`;
      const generated = await request("/key/generate", {
        key_alias: alias,
        duration: configuration.duration,
        models: ["openai/gpt-4.1-mini"],
        allowed_routes: ["/v1/chat/completions", "/v1/models"],
        max_budget: configuration.maxBudgetUsd,
        rpm_limit: 6,
        tpm_limit: 8_000,
        metadata: {
          purpose: "luna-managed-intelligence",
          household_id: input.householdId,
          trusted_device_id: input.deviceId,
        },
      });
      if (typeof generated.key !== "string" || generated.key.trim() === "") {
        throw new Error("Managed gateway provisioning returned an invalid credential.");
      }
      return { alias, credential: generated.key };
    },
    async revokeGatewayAccess(credential: string) {
      await request("/key/delete", { keys: [credential] });
    },
    async revokeGatewayAccessByAlias(alias: string) {
      await request("/key/delete", { key_aliases: [alias] });
    },
  };
}
