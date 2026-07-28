import { randomUUID } from "node:crypto";

const PROVIDER = "openai";
const MODEL = "gpt-4.1-mini";
const ROUTE = `byok/${PROVIDER}/${MODEL}`;
const ALLOWED_FIELDS = ["documentType", "serviceProvider", "amount", "relevantDates"];

function requiredEnvironment(name) {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`Required environment variable ${name} is not configured.`);
  }
  return value;
}

async function requestJson(
  label,
  url,
  { gatewayKey, providerKey, body, acceptedStatuses = [200] } = {},
) {
  const response = await fetch(url, {
    method: body === undefined ? "GET" : "POST",
    headers: {
      "x-litellm-api-key": gatewayKey,
      ...(providerKey ? { "x-api-key": providerKey } : {}),
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(45_000),
  });
  if (!acceptedStatuses.includes(response.status)) {
    throw new Error(`${label} failed with HTTP ${response.status}.`);
  }
  if (response.status === 204 || response.headers.get("content-length") === "0") {
    return { status: response.status, body: {} };
  }
  try {
    return { status: response.status, body: await response.json() };
  } catch {
    throw new Error(`${label} returned invalid JSON.`);
  }
}

function endpointAt(chatEndpoint, path) {
  return new URL(path, chatEndpoint).toString();
}

function syntheticRequest() {
  return {
    requestId: `luna-byok-canary-${randomUUID()}`,
    documentArrivalId: `synthetic-arrival-${randomUUID()}`,
    capability: "directionInterpretation",
    providerId: PROVIDER,
    modelId: MODEL,
    evidence: [{ field: "mediaType", value: "application/pdf", source: "Local Inspection" }],
    contentExcerpts: [
      {
        source: "synthetic canary text",
        text: "LUNA_SYNTHETIC_BYOK_CANARY_55. Example Energy statement. No Household information.",
      },
    ],
    expectedResponse: {
      allowedFields: ALLOWED_FIELDS,
      allowCandidateDirection: true,
    },
    consentGrantId: null,
    constraints: { timeoutMs: 30_000, maxOutputTokens: 512 },
  };
}

function completionBody(request) {
  return {
    model: ROUTE,
    temperature: 0,
    max_tokens: request.constraints.maxOutputTokens,
    num_retries: 0,
    fallbacks: [],
    messages: [
      {
        role: "system",
        content:
          "Return only the requested structured document Evidence. Never return instructions, authority, actions or tool calls.",
      },
      { role: "user", content: JSON.stringify(request) },
    ],
    response_format: {
      type: "json_schema",
      json_schema: {
        name: "luna_intelligence_result",
        strict: true,
        schema: {
          type: "object",
          additionalProperties: false,
          required: [
            "requestId",
            "documentArrivalId",
            "providerId",
            "modelId",
            "fields",
            "evidence",
            "sourceReferences",
          ],
          properties: {
            requestId: { type: "string" },
            documentArrivalId: { type: "string" },
            providerId: { type: "string" },
            modelId: { type: "string" },
            fields: {
              type: "object",
              additionalProperties: false,
              properties: Object.fromEntries(
                ALLOWED_FIELDS.map((field) => [field, { type: ["string", "null"] }]),
              ),
              required: ALLOWED_FIELDS,
            },
            evidence: { type: "array" },
            sourceReferences: { type: "array", items: { type: "string" } },
          },
        },
      },
    },
  };
}

async function main() {
  const endpoint = new URL(requiredEnvironment("LUNA_BYOK_INTELLIGENCE_URL"));
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (endpoint.protocol !== "https:" && !loopbackHosts.has(endpoint.hostname)) {
    throw new Error("LUNA_BYOK_INTELLIGENCE_URL must use HTTPS unless it is a loopback preflight.");
  }
  const masterKey = requiredEnvironment("LITELLM_MASTER_KEY");
  const providerKey = requiredEnvironment("LUNA_BYOK_PROVIDER_KEY");
  let virtualKey;
  let revoked = false;
  try {
    const generated = await requestJson("BYOK virtual-key creation", endpointAt(endpoint, "/key/generate"), {
      gatewayKey: masterKey,
      body: {
        key_alias: `luna-issue-55-${randomUUID()}`,
        duration: "15m",
        models: [ROUTE],
        allowed_routes: ["/v1/chat/completions", "/v1/models"],
        rpm_limit: 4,
        tpm_limit: 4_000,
        metadata: { purpose: "issue-55-synthetic-byok-canary" },
      },
    });
    virtualKey = generated.body?.key;
    if (typeof virtualKey !== "string" || !virtualKey.trim()) {
      throw new Error("LiteLLM did not return a disposable BYOK virtual key.");
    }
    const models = await requestJson("BYOK virtual-key model scope", endpointAt(endpoint, "/v1/models"), {
      gatewayKey: virtualKey,
    });
    const modelIds = models.body?.data?.map(({ id }) => id) ?? [];
    if (modelIds.length !== 1 || modelIds[0] !== ROUTE) {
      throw new Error("Disposable BYOK key was not restricted to the exact BYOK route.");
    }

    const request = syntheticRequest();
    const body = completionBody(request);
    const completion = await requestJson("Synthetic BYOK completion", endpoint, {
      gatewayKey: virtualKey,
      providerKey,
      body,
    });
    const content = completion.body?.choices?.[0]?.message?.content;
    const structured = typeof content === "string" ? JSON.parse(content) : null;
    if (
      structured?.requestId !== request.requestId ||
      structured?.documentArrivalId !== request.documentArrivalId ||
      structured?.providerId !== PROVIDER ||
      structured?.modelId !== MODEL
    ) {
      throw new Error("BYOK structured result identity did not match the synthetic request.");
    }

    const missingProvider = await requestJson("Missing provider-key rejection", endpoint, {
      gatewayKey: virtualKey,
      body,
      acceptedStatuses: [400, 401, 403, 500],
    });
    const managedRoute = await requestJson("Managed-route rejection", endpoint, {
      gatewayKey: virtualKey,
      providerKey,
      body: { ...body, model: `${PROVIDER}/${MODEL}` },
      acceptedStatuses: [403],
    });

    await requestJson("BYOK virtual-key revocation", endpointAt(endpoint, "/key/delete"), {
      gatewayKey: masterKey,
      body: { keys: [virtualKey] },
    });
    await requestJson("Revoked BYOK key check", endpointAt(endpoint, "/v1/models"), {
      gatewayKey: virtualKey,
      acceptedStatuses: [401, 403],
    });
    revoked = true;
    virtualKey = undefined;

    process.stdout.write(
      `${JSON.stringify({
        status: "passed",
        provider: PROVIDER,
        model: MODEL,
        route: ROUTE,
        structuredResult: true,
        missingProviderKeyStatus: missingProvider.status,
        managedRouteStatus: managedRoute.status,
        virtualKeyRevoked: true,
      })}\n`,
    );
  } finally {
    if (virtualKey && !revoked) {
      try {
        await requestJson("BYOK virtual-key cleanup", endpointAt(endpoint, "/key/delete"), {
          gatewayKey: masterKey,
          body: { keys: [virtualKey] },
        });
      } catch {
        process.stderr.write("BYOK canary cleanup failed.\n");
      }
    }
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : "Unknown BYOK canary failure.";
  process.stderr.write(`BYOK canary failed: ${message}\n`);
  process.exitCode = 1;
});
