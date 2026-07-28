import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const GATEWAY_KEY = "sk-luna-gateway-test-only";
const VIRTUAL_KEY = "sk-byok-virtual-test-only";
const PROVIDER_KEY = "sk-customer-provider-test-only";

function readJson(request) {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      try {
        resolve(body ? JSON.parse(body) : {});
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

async function startGateway() {
  const observations = [];
  let revoked = false;
  const server = createServer(async (request, response) => {
    const body = await readJson(request);
    observations.push({ headers: request.headers, method: request.method, path: request.url, body });
    const send = (status, value) => {
      response.writeHead(status, { "content-type": "application/json" });
      response.end(JSON.stringify(value));
    };
    if (request.method === "POST" && request.url === "/key/generate") {
      send(200, { key: VIRTUAL_KEY });
      return;
    }
    if (request.method === "GET" && request.url === "/v1/models") {
      if (revoked) {
        send(401, { error: { message: "revoked" } });
      } else {
        send(200, { data: [{ id: "byok/openai/gpt-4.1-mini" }], object: "list" });
      }
      return;
    }
    if (request.method === "POST" && request.url === "/key/delete") {
      revoked = true;
      send(200, { deleted_keys: ["hashed-key-id"] });
      return;
    }
    if (request.method !== "POST" || request.url !== "/v1/chat/completions") {
      send(404, { error: "not found" });
      return;
    }
    if (body.model === "openai/gpt-4.1-mini") {
      send(403, { error: { message: "route outside virtual-key scope" } });
      return;
    }
    if (!request.headers["x-api-key"]) {
      send(500, { error: { message: "provider key unavailable" } });
      return;
    }
    const prompt = JSON.parse(body.messages[1].content);
    send(200, {
        model: "gpt-4.1-mini",
        choices: [
          {
            message: {
              content: JSON.stringify({
                requestId: prompt.requestId,
                documentArrivalId: prompt.documentArrivalId,
                providerId: "openai",
                modelId: "gpt-4.1-mini",
                fields: {
                  documentType: "utility statement",
                  serviceProvider: "Example Energy",
                  amount: "42.00",
                  relevantDates: "2026-07-01",
                },
                evidence: [],
                sourceReferences: [],
              }),
            },
          },
        ],
        usage: { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 },
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    endpoint: `http://127.0.0.1:${port}/v1/chat/completions`,
    observations,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

function runCanary(endpoint) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["ops/litellm/byok-canary.mjs"], {
      cwd: new URL("../..", import.meta.url),
      env: {
        ...process.env,
        LUNA_BYOK_INTELLIGENCE_URL: endpoint,
        LITELLM_MASTER_KEY: GATEWAY_KEY,
        LUNA_BYOK_PROVIDER_KEY: PROVIDER_KEY,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

test("BYOK canary keeps Luna gateway authentication separate from the provider credential", async () => {
  const gateway = await startGateway();
  try {
    const result = await runCanary(gateway.endpoint);

    assert.equal(result.code, 0, result.stderr);
    assert.equal(result.stderr, "");
    assert.doesNotMatch(result.stdout, new RegExp(GATEWAY_KEY));
    assert.doesNotMatch(result.stdout, new RegExp(VIRTUAL_KEY));
    assert.doesNotMatch(result.stdout, new RegExp(PROVIDER_KEY));
    assert.equal(gateway.observations.length, 7);
    assert.equal(gateway.observations[0].headers["x-litellm-api-key"], GATEWAY_KEY);
    assert.deepEqual(gateway.observations[0].body.models, ["byok/openai/gpt-4.1-mini"]);
    assert.equal(gateway.observations[2].headers["x-litellm-api-key"], VIRTUAL_KEY);
    assert.equal(gateway.observations[2].headers["x-api-key"], PROVIDER_KEY);
    assert.equal(gateway.observations[2].headers.authorization, undefined);
    assert.equal(gateway.observations[2].body.model, "byok/openai/gpt-4.1-mini");
    assert.equal(gateway.observations[3].headers["x-api-key"], undefined);
    assert.equal(gateway.observations[4].body.model, "openai/gpt-4.1-mini");
    assert.deepEqual(JSON.parse(result.stdout), {
      status: "passed",
      provider: "openai",
      model: "gpt-4.1-mini",
      route: "byok/openai/gpt-4.1-mini",
      structuredResult: true,
      missingProviderKeyStatus: 500,
      managedRouteStatus: 403,
      virtualKeyRevoked: true,
    });
  } finally {
    await gateway.close();
  }
});

test("BYOK canary refuses to send a customer provider credential over cleartext", async () => {
  const result = await runCanary("http://gateway.example.com/v1/chat/completions");

  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /HTTPS unless it is a loopback preflight/);
  assert.doesNotMatch(result.stderr, new RegExp(GATEWAY_KEY));
  assert.doesNotMatch(result.stderr, new RegExp(PROVIDER_KEY));
});
