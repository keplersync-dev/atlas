// Drives the Atlas windows already running under `pnpm tauri dev` (and `pnpm dev:profile`),
// started with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222. It
// attaches rather than launching, so it never opens app data by itself.
export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      browserName: "MicrosoftEdge",
      "ms:edgeOptions": { debuggerAddress: "localhost:9222" },
      // BiDi injects a page of its own and blanked an Atlas window; classic doesn't.
      "wdio:enforceWebDriverClassic": true,
    },
  ],
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 20 * 60 * 1000 },
};
