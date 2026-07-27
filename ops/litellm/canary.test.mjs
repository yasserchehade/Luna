import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const MASTER_KEY = "sk-master-test-only";
const VIRTUAL_KEY = "sk-virtual-test-only";

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

function sendJson(response, status, body) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

async function startGateway() {
  const observations = [];
  let revoked = false;
  const server = createServer(async (request, response) => {
    const body = await readJson(request);
    observations.push({
      method: request.method,
      path: request.url,
      authorization: request.headers.authorization,
      body,
    });

    if (request.method === "POST" && request.url === "/key/generate") {
      sendJson(response, 200, { key: VIRTUAL_KEY, key_name: "hashed-key-id" });
      return;
    }
    if (request.method === "GET" && request.url === "/v1/models") {
      if (revoked) {
        sendJson(response, 401, { error: { message: "revoked" } });
      } else {
        sendJson(response, 200, {
          object: "list",
          data: [{ id: "openai/gpt-4.1-mini", object: "model" }],
        });
      }
      return;
    }
    if (request.method === "POST" && request.url === "/v1/chat/completions") {
      const prompt = JSON.parse(body.messages[1].content);
      sendJson(response, 200, {
        id: "synthetic-completion",
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
        usage: {
          prompt_tokens: 123,
          completion_tokens: 45,
          total_tokens: 168,
        },
      });
      return;
    }
    if (request.method === "POST" && request.url === "/key/delete") {
      revoked = true;
      sendJson(response, 200, { deleted_keys: ["hashed-key-id"] });
      return;
    }
    sendJson(response, 404, { error: "not found" });
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
    const child = spawn(process.execPath, ["ops/litellm/canary.mjs"], {
      cwd: new URL("../..", import.meta.url),
      env: {
        ...process.env,
        LUNA_MANAGED_INTELLIGENCE_URL: endpoint,
        LITELLM_MASTER_KEY: MASTER_KEY,
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

test("canary proves the exact route, structured result, usage, and revocation", async () => {
  const gateway = await startGateway();
  try {
    const result = await runCanary(gateway.endpoint);

    assert.equal(result.code, 0, result.stderr);
    assert.equal(result.stderr, "");
    assert.doesNotMatch(result.stdout, new RegExp(MASTER_KEY));
    assert.doesNotMatch(result.stdout, new RegExp(VIRTUAL_KEY));

    const evidence = JSON.parse(result.stdout);
    assert.deepEqual(evidence, {
      status: "passed",
      provider: "openai",
      model: "gpt-4.1-mini",
      route: "openai/gpt-4.1-mini",
      upstreamModel: "gpt-4.1-mini",
      structuredResult: true,
      usage: { inputTokens: 123, outputTokens: 45, totalTokens: 168 },
      virtualKeyRevoked: true,
    });

    assert.deepEqual(
      gateway.observations.map(({ method, path }) => `${method} ${path}`),
      [
        "POST /key/generate",
        "GET /v1/models",
        "POST /v1/chat/completions",
        "POST /key/delete",
        "GET /v1/models",
      ],
    );
    assert.deepEqual(gateway.observations[0].body.models, ["openai/gpt-4.1-mini"]);
    assert.deepEqual(gateway.observations[0].body.allowed_routes, [
      "/v1/chat/completions",
      "/v1/models",
    ]);
    assert.equal(gateway.observations[2].body.model, "openai/gpt-4.1-mini");
    assert.equal(gateway.observations[2].body.num_retries, 0);
    assert.deepEqual(gateway.observations[2].body.fallbacks, []);
    assert.equal(gateway.observations[2].body.response_format.json_schema.strict, true);
  } finally {
    await gateway.close();
  }
});

test("canary refuses cleartext non-loopback gateways", async () => {
  const result = await runCanary("http://gateway.example.com/v1/chat/completions");

  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /HTTPS unless it is a loopback preflight/);
  assert.doesNotMatch(result.stderr, new RegExp(MASTER_KEY));
});
