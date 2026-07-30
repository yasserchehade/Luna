import path from "node:path";

const executable = process.platform === "win32" ? "luna-desktop.exe" : "luna-desktop";
const application = process.env.LUNA_LIVE_CANARY_BINARY
  ? path.resolve(process.env.LUNA_LIVE_CANARY_BINARY)
  : path.resolve("src-tauri", "target", "live-canary", "debug", executable);
const tauriServiceOptions = {
  appBinaryPath: application,
  driverProvider: "embedded",
  statusPollTimeout: 15_000,
} as const;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./e2e-live/**/*.spec.ts"],
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
  waitforTimeout: 20_000,
  mochaOpts: {
    timeout: 300_000,
  },
};
