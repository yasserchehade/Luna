import path from "node:path";

const executable = process.platform === "win32" ? "luna-desktop.exe" : "luna-desktop";
const application = process.env.LUNA_E2E_BINARY
  ? path.resolve(process.env.LUNA_E2E_BINARY)
  : path.resolve("src-tauri", "target", "e2e", "debug", executable);
const tauriServiceOptions = {
  appBinaryPath: application,
  driverProvider: "embedded",
  statusPollTimeout: 10_000,
} as const;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./e2e/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [{
    browserName: "tauri",
    "tauri:options": { application },
    "wdio:tauriServiceOptions": tauriServiceOptions,
  } as WebdriverIO.Capabilities],
  services: [["@wdio/tauri-service", tauriServiceOptions]],
  framework: "mocha",
  reporters: ["spec"],
  logLevel: "warn",
  waitforTimeout: 10_000,
  mochaOpts: {
    timeout: 60_000,
  },
};
