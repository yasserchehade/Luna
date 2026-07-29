import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

function renderedCompose(files = ["ops/litellm/compose.yaml"]) {
  const fileArguments = files.flatMap((file) => ["-f", file]);
  const result = spawnSync(
    "docker",
    ["compose", ...fileArguments, "config", "--format", "json"],
    {
      cwd: new URL("../..", import.meta.url),
      encoding: "utf8",
      env: {
        ...process.env,
        OPENAI_API_KEY: "managed-placeholder",
        LITELLM_MASTER_KEY: "sk-master-placeholder",
        LITELLM_DATABASE_PASSWORD: "database-placeholder",
        DATABASE_URL: "postgresql://llmproxy:database-placeholder@database:5432/litellm",
        LUNA_CLOUDFLARE_TUNNEL_TOKEN_FILE: "ops/litellm/config.yaml",
      },
    },
  );
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("rendered deployment isolates BYOK routes from Luna-funded provider credentials", () => {
  const compose = renderedCompose();
  const managed = compose.services.gateway;
  const byok = compose.services["byok-gateway"];

  assert.ok(byok, "BYOK gateway service must exist");
  assert.equal(managed.environment.OPENAI_API_KEY, "managed-placeholder");
  assert.equal(byok.environment.OPENAI_API_KEY, undefined);
  assert.equal(byok.environment.LITELLM_MASTER_KEY, "sk-master-placeholder");
  assert.notDeepEqual(byok.command, managed.command);
  assert.match(byok.command.join(" "), /byok-config\.yaml/);
});

test("pre-production ingress keeps gateway ports on loopback and the tunnel token out of process arguments", () => {
  const compose = renderedCompose([
    "ops/litellm/compose.yaml",
    "ops/litellm/compose.cloudflare.yaml",
  ]);
  const tunnel = compose.services.cloudflared;
  const publicIngress = compose.services["managed-public-ingress"];
  const adminIngress = compose.services["managed-admin-ingress"];

  assert.equal(
    tunnel.image,
    "cloudflare/cloudflared:2026.7.2@sha256:4f6655284ab3d252b7f28fedb19fe6c8fc82ee5b1295c20ac74d475e5398a52d",
  );
  assert.doesNotMatch(tunnel.command.join(" "), /token-placeholder|--token(?:\s|$)/);
  assert.match(tunnel.command.join(" "), /--token-file \/run\/secrets\/cloudflare_tunnel_token/);
  assert.equal(tunnel.environment, undefined);
  assert.equal(tunnel.ports, undefined);
  assert.deepEqual(Object.keys(tunnel.networks), ["ingress"]);

  for (const ingress of [publicIngress, adminIngress]) {
    assert.equal(
      ingress.image,
      "caddy:2.10.2-alpine@sha256:4c6e91c6ed0e2fa03efd5b44747b625fec79bc9cd06ac5235a779726618e530d",
    );
    assert.equal(ingress.ports, undefined);
    assert.equal(ingress.read_only, true);
    assert.deepEqual(ingress.cap_drop, ["ALL"]);
    assert.deepEqual(Object.keys(ingress.networks), ["ingress"]);
  }

  assert.deepEqual(compose.services.gateway.ports[0], {
    mode: "ingress",
    target: 4000,
    published: "4000",
    protocol: "tcp",
    host_ip: "127.0.0.1",
  });
});
