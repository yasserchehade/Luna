import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

function renderedCompose() {
  const result = spawnSync(
    "docker",
    ["compose", "-f", "ops/litellm/compose.yaml", "config", "--format", "json"],
    {
      cwd: new URL("../..", import.meta.url),
      encoding: "utf8",
      env: {
        ...process.env,
        OPENAI_API_KEY: "managed-placeholder",
        LITELLM_MASTER_KEY: "sk-master-placeholder",
        LITELLM_DATABASE_PASSWORD: "database-placeholder",
        DATABASE_URL: "postgresql://llmproxy:database-placeholder@database:5432/litellm",
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
