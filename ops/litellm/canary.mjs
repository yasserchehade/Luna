import { randomUUID } from "node:crypto";

const PROVIDER = "openai";
const MODEL = "gpt-4.1-mini";
const ROUTE = `${PROVIDER}/${MODEL}`;
const ALLOWED_FIELDS = ["documentType", "serviceProvider", "amount", "relevantDates"];

function requiredEnvironment(name) {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`Required environment variable ${name} is not configured.`);
  }
  return value;
}

async function requestJson(label, url, { bearer, body, acceptedStatuses = [200] } = {}) {
  const response = await fetch(url, {
    method: body === undefined ? "GET" : "POST",
    headers: {
      authorization: `Bearer ${bearer}`,
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(45_000),
  });
  if (!acceptedStatuses.includes(response.status)) {
    throw new Error(`${label} failed with HTTP ${response.status}.`);
  }
  if (response.status === 204 || response.headers.get("content-length") === "0") {
    return {};
  }
  try {
    return await response.json();
  } catch {
    throw new Error(`${label} returned invalid JSON.`);
  }
}

function endpointAt(chatEndpoint, path) {
  return new URL(path, chatEndpoint).toString();
}

function syntheticRequest() {
  const requestId = `luna-canary-${randomUUID()}`;
  const documentArrivalId = `synthetic-arrival-${randomUUID()}`;
  return {
    requestId,
    documentArrivalId,
    capability: "directionInterpretation",
    providerId: PROVIDER,
    modelId: MODEL,
    evidence: [{ field: "mediaType", value: "application/pdf", source: "Local Inspection" }],
    contentExcerpts: [
      {
        source: "synthetic canary text",
        text: "LUNA_SYNTHETIC_CANARY_53. Example Energy utility statement dated 2026-07-01. Amount due AUD 42.00. No Household information.",
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
  const fieldProperties = Object.fromEntries(
    ALLOWED_FIELDS.map((field) => [field, { type: ["string", "null"] }]),
  );
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
              properties: fieldProperties,
              required: ALLOWED_FIELDS,
            },
            evidence: {
              type: "array",
              items: {
                type: "object",
                additionalProperties: false,
                required: ["field", "value", "sourceReference"],
                properties: {
                  field: { type: "string" },
                  value: { type: "string" },
                  sourceReference: { type: ["string", "null"] },
                },
              },
            },
            sourceReferences: { type: "array", items: { type: "string" } },
          },
        },
      },
    },
  };
}

function requireStructuredResult(response, request) {
  const content = response?.choices?.[0]?.message?.content;
  if (typeof content !== "string") {
    throw new Error("Completion did not contain a structured message.");
  }
  let result;
  try {
    result = JSON.parse(content);
  } catch {
    throw new Error("Completion message was not valid structured JSON.");
  }
  const expectedKeys = [
    "documentArrivalId",
    "evidence",
    "fields",
    "modelId",
    "providerId",
    "requestId",
    "sourceReferences",
  ];
  if (JSON.stringify(Object.keys(result).sort()) !== JSON.stringify(expectedKeys)) {
    throw new Error("Structured result fields did not match Luna's contract.");
  }
  if (
    result.requestId !== request.requestId ||
    result.documentArrivalId !== request.documentArrivalId ||
    result.providerId !== PROVIDER ||
    result.modelId !== MODEL
  ) {
    throw new Error("Structured result identity did not match the canary request.");
  }
  if (
    !result.fields ||
    JSON.stringify(Object.keys(result.fields).sort()) !== JSON.stringify([...ALLOWED_FIELDS].sort()) ||
    Object.values(result.fields).some((value) => value !== null && typeof value !== "string") ||
    !Array.isArray(result.evidence) ||
    !Array.isArray(result.sourceReferences)
  ) {
    throw new Error("Structured result content did not match Luna's bounded schema.");
  }
}

function requireUsage(response) {
  const inputTokens = response?.usage?.prompt_tokens;
  const outputTokens = response?.usage?.completion_tokens;
  const totalTokens = response?.usage?.total_tokens;
  if (![inputTokens, outputTokens, totalTokens].every(Number.isFinite)) {
    throw new Error("Completion did not contain privacy-safe token usage metadata.");
  }
  return { inputTokens, outputTokens, totalTokens };
}

async function main() {
  const chatEndpoint = new URL(requiredEnvironment("LUNA_MANAGED_INTELLIGENCE_URL"));
  if (!chatEndpoint.pathname.endsWith("/v1/chat/completions")) {
    throw new Error("LUNA_MANAGED_INTELLIGENCE_URL must end with /v1/chat/completions.");
  }
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (chatEndpoint.protocol !== "https:" && !loopbackHosts.has(chatEndpoint.hostname)) {
    throw new Error(
      "LUNA_MANAGED_INTELLIGENCE_URL must use HTTPS unless it is a loopback preflight.",
    );
  }
  const masterKey = requiredEnvironment("LITELLM_MASTER_KEY");
  const generateUrl = endpointAt(chatEndpoint, "/key/generate");
  const modelsUrl = endpointAt(chatEndpoint, "/v1/models");
  const deleteUrl = endpointAt(chatEndpoint, "/key/delete");
  let virtualKey;
  let virtualKeyRevoked = false;

  try {
    const generated = await requestJson("Virtual-key creation", generateUrl, {
      bearer: masterKey,
      body: {
        key_alias: `luna-issue-53-${randomUUID()}`,
        duration: "15m",
        models: [ROUTE],
        allowed_routes: ["/v1/chat/completions", "/v1/models"],
        max_budget: 0.1,
        rpm_limit: 2,
        tpm_limit: 4_000,
        metadata: { purpose: "issue-53-synthetic-canary" },
      },
    });
    virtualKey = generated?.key;
    if (typeof virtualKey !== "string" || !virtualKey.trim()) {
      throw new Error("LiteLLM did not return a disposable virtual key.");
    }

    const models = await requestJson("Virtual-key model scope", modelsUrl, {
      bearer: virtualKey,
    });
    const modelIds = models?.data?.map(({ id }) => id) ?? [];
    if (modelIds.length !== 1 || modelIds[0] !== ROUTE) {
      throw new Error("Disposable virtual key was not restricted to the exact approved route.");
    }

    const request = syntheticRequest();
    const completion = await requestJson("Synthetic completion", chatEndpoint, {
      bearer: virtualKey,
      body: completionBody(request),
    });
    const upstreamModel = completion?.model;
    if (
      typeof upstreamModel !== "string" ||
      (upstreamModel !== MODEL && !upstreamModel.startsWith(`${MODEL}-`))
    ) {
      throw new Error("Completion did not report the approved OpenAI model route.");
    }
    requireStructuredResult(completion, request);
    const usage = requireUsage(completion);

    await requestJson("Virtual-key revocation", deleteUrl, {
      bearer: masterKey,
      body: { keys: [virtualKey] },
    });
    const revokedResponse = await requestJson("Revoked-key check", modelsUrl, {
      bearer: virtualKey,
      acceptedStatuses: [401, 403],
    });
    void revokedResponse;
    virtualKeyRevoked = true;
    virtualKey = undefined;

    process.stdout.write(
      `${JSON.stringify({
        status: "passed",
        provider: PROVIDER,
        model: MODEL,
        route: ROUTE,
        upstreamModel,
        structuredResult: true,
        usage,
        virtualKeyRevoked,
      })}\n`,
    );
  } finally {
    if (virtualKey && !virtualKeyRevoked) {
      try {
        await requestJson("Virtual-key cleanup", deleteUrl, {
          bearer: masterKey,
          body: { keys: [virtualKey] },
        });
      } catch {
        process.stderr.write("Canary failed and automatic virtual-key cleanup also failed.\n");
      }
    }
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : "Unknown canary failure.";
  process.stderr.write(`Canary failed: ${message}\n`);
  process.exitCode = 1;
});
