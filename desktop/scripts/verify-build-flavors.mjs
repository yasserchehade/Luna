import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

const desktopRoot = process.cwd();
const executableName = process.platform === "win32" ? "luna-desktop.exe" : "luna-desktop";
const e2eExecutable = path.join(desktopRoot, "src-tauri", "target", "e2e", "debug", executableName);
const normalExecutable = path.join(desktopRoot, "src-tauri", "target", "debug", executableName);

const containsE2eMarker = (bytes) => Buffer.from(bytes).includes(Buffer.from("e2eAccountService"));

const [e2eBytes, normalBytes] = await Promise.all([
  readFile(e2eExecutable),
  readFile(normalExecutable),
]);

assert.ok(containsE2eMarker(e2eBytes), "the E2E binary should contain the E2E account service");
assert.ok(!containsE2eMarker(normalBytes), "the normal binary must not contain the E2E account service");

console.log("Build flavors are isolated.");
