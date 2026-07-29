import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, "..", "..");
const script = path.join(scriptDirectory, "provision-managed-canary.mjs");
const safeCredentialPath = path.join(os.tmpdir(), "luna-managed-canary-test.clixml");
const commonArguments = [
  "--project-ref",
  "aaaaaaaaaaaaaaaaaaaa",
  "--email",
  "canary@example.com",
  "--credential-path",
  safeCredentialPath,
];

test("managed canary provisioning requires an explicit live confirmation", () => {
  const result = run(commonArguments);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Refusing to change the linked Supabase project/);
  assert.doesNotMatch(result.stdout, /password|totp/i);
});

test("managed canary provisioning rejects an invalid project reference before any live call", () => {
  const result = run([
    "--confirm-live",
    "--project-ref",
    "not-a-project",
    "--email",
    "canary@example.com",
    "--credential-path",
    safeCredentialPath,
  ]);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /not a valid Supabase project reference/);
});

test("managed canary credentials cannot be written inside the public repository", () => {
  const result = run([
    "--confirm-live",
    ...commonArguments.slice(0, -1),
    path.join(repositoryRoot, "managed-canary.clixml"),
  ]);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /must be outside the public repository/);
});

function run(arguments_) {
  return spawnSync(process.execPath, [script, ...arguments_], {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  });
}
