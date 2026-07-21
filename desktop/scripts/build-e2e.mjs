import { spawnSync } from "node:child_process";
import path from "node:path";

const command = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const targetDir = path.resolve("src-tauri", "target", "e2e");
const result = spawnSync(command, [
  "exec",
  "tauri",
  "build",
  "--debug",
  "--no-bundle",
  "--features",
  "e2e",
  "--config",
  "src-tauri/tauri.e2e.conf.json",
], {
  env: { ...process.env, CARGO_TARGET_DIR: targetDir },
  shell: process.platform === "win32",
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
