import { spawnSync } from "node:child_process";
import path from "node:path";

const command = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const targetDir = path.resolve("src-tauri", "target", "live-canary");
const result = spawnSync(command, [
  "exec",
  "tauri",
  "build",
  "--debug",
  "--no-bundle",
  "--features",
  "live-canary",
  "--config",
  "src-tauri/tauri.live-canary.conf.json",
], {
  env: { ...process.env, CARGO_TARGET_DIR: targetDir },
  shell: process.platform === "win32",
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
