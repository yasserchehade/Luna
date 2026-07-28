type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

type ManagedAccessConfiguration = {
  adminEndpoint: string;
  endpoint: string;
  masterKey: string;
  durationHours: number;
  rpmLimit: number;
  tpmLimit: number;
  fetch: FetchLike;
  now?: () => Date;
};

export function createLiteLlmManagedAccessClient(configuration: ManagedAccessConfiguration) {
  const chatEndpoint = new URL(configuration.endpoint);
  const adminEndpoint = new URL(configuration.adminEndpoint);
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  validateSecureEndpoint(chatEndpoint, loopbackHosts, "managed gateway");
  validateSecureEndpoint(adminEndpoint, loopbackHosts, "managed gateway administration");
  if (!chatEndpoint.pathname.endsWith("/v1/chat/completions")) {
    throw new Error("The managed gateway endpoint is invalid.");
  }
  if (
    !Number.isInteger(configuration.durationHours)
    || configuration.durationHours < 2
    || configuration.durationHours > 24
  ) {
    throw new Error("The managed gateway credential duration is invalid.");
  }
  if (!Number.isInteger(configuration.rpmLimit) || configuration.rpmLimit <= 0) {
    throw new Error("The managed gateway request limit is invalid.");
  }
  if (!Number.isInteger(configuration.tpmLimit) || configuration.tpmLimit <= 0) {
    throw new Error("The managed gateway token limit is invalid.");
  }

  const send = (path: string, method: "GET" | "POST", body?: Record<string, unknown>) => (
    configuration.fetch(new URL(path, adminEndpoint).toString(), {
      method,
      headers: {
        authorization: `Bearer ${configuration.masterKey}`,
        ...(body ? { "content-type": "application/json" } : {}),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
    })
  );

  const requireJson = async (
    path: string,
    method: "GET" | "POST",
    body?: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const response = await send(path, method, body);
    if (!response.ok) throw new Error("Managed gateway provisioning is unavailable.");
    return response.json() as Promise<Record<string, unknown>>;
  };

  const ensureHouseholdBudget = async (input: {
    householdId: string;
    budgetScopeId: string;
    maxBudgetUsd: number;
  }): Promise<string> => {
    if (!Number.isFinite(input.maxBudgetUsd) || input.maxBudgetUsd <= 0 || input.maxBudgetUsd > 100) {
      throw new Error("The managed Household budget is invalid.");
    }
    const teamId = `luna-household-${input.householdId}-${input.budgetScopeId}`;
    const team = {
      team_id: teamId,
      team_alias: teamId,
      models: ["openai/gpt-4.1-mini"],
      max_budget: input.maxBudgetUsd,
      rpm_limit: configuration.rpmLimit,
      tpm_limit: configuration.tpmLimit,
      metadata: { purpose: "luna-managed-intelligence", household_id: input.householdId },
    };
    const created = await send("/team/new", "POST", team);
    if (!created.ok) {
      await requireJson("/team/update", "POST", team);
    }
    return teamId;
  };

  return {
    async createGatewayAccess(input: {
      householdId: string;
      deviceId: string;
      alias: string;
      budgetScopeId: string;
      maxBudgetUsd: number;
    }) {
      const teamId = await ensureHouseholdBudget(input);
      const generated = await requireJson("/key/generate", "POST", {
        key_alias: input.alias,
        duration: `${configuration.durationHours}h`,
        team_id: teamId,
        models: ["openai/gpt-4.1-mini"],
        allowed_routes: ["/v1/chat/completions"],
        rpm_limit: configuration.rpmLimit,
        tpm_limit: configuration.tpmLimit,
        metadata: {
          purpose: "luna-managed-intelligence",
          household_id: input.householdId,
          trusted_device_id: input.deviceId,
          budget_scope_id: input.budgetScopeId,
        },
      });
      if (typeof generated.key !== "string" || generated.key.trim() === "") {
        throw new Error("Managed gateway provisioning returned an invalid credential.");
      }
      const calculatedExpiry = new Date(
        (configuration.now?.() ?? new Date()).getTime() + configuration.durationHours * 60 * 60 * 1_000,
      ).toISOString();
      const expiresAt = typeof generated.expires === "string" && !Number.isNaN(Date.parse(generated.expires))
        ? generated.expires
        : calculatedExpiry;
      return { alias: input.alias, credential: generated.key, expiresAt };
    },
    async revokeGatewayAccess(credential: string) {
      await requireJson("/key/delete", "POST", { keys: [credential] });
    },
    async revokeGatewayAccessByAlias(alias: string) {
      const response = await send("/key/delete", "POST", { key_aliases: [alias] });
      if (!response.ok && response.status !== 404) {
        throw new Error("Managed gateway revocation is unavailable.");
      }
    },
  };
}

function validateSecureEndpoint(endpoint: URL, loopbackHosts: Set<string>, name: string): void {
  if (endpoint.protocol !== "https:" && !loopbackHosts.has(endpoint.hostname)) {
    throw new Error(`The ${name} must use HTTPS unless it is loopback-only.`);
  }
}
