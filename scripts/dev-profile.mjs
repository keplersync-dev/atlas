// Runs another copy of Atlas with its own profile next to `pnpm tauri dev`, and restarts
// it whenever that rebuilds the app, so both windows stay on the same code. The UI comes
// from the same Vite server, so its changes reach both without a restart.
//
//   pnpm dev:profile        profile "2"
//   pnpm dev:profile 3      profile "3"
//
// It runs a copy of the exe: the running original can't be replaced, which would stop
// `pnpm tauri dev` from rebuilding. Closing the window ends this script too.

import { spawn } from "node:child_process";
import { copyFileSync, statSync, watchFile } from "node:fs";
import { join } from "node:path";

const profile = process.argv[2] ?? "2";
if (!/^[A-Za-z0-9_-]+$/.test(profile)) {
  console.error("A profile name is letters, digits, - or _.");
  process.exit(1);
}
const dir = join(process.env.CARGO_TARGET_DIR ?? "src-tauri/target", "debug");
const exe = join(dir, "atlas.exe");
const copy = join(dir, `atlas-${profile}.exe`);

let child = null;
let restarting = false;

function start() {
  copyFileSync(exe, copy);
  child = spawn(copy, [], {
    stdio: "inherit",
    env: { ...process.env, ATLAS_PROFILE: profile },
  });
  child.on("exit", () => {
    // Closed by the user, not by a restart: stop, like `pnpm tauri dev` does.
    if (!restarting) process.exit(0);
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Wait until the linker has finished writing the exe: its size stops changing. */
async function settled() {
  let last = -1;
  for (;;) {
    await sleep(1000);
    const size = statSync(exe, { throwIfNoEntry: false })?.size ?? -1;
    if (size > 0 && size === last) return;
    last = size;
  }
}

async function restart() {
  if (restarting) return;
  restarting = true;
  await settled();
  console.log(`[dev-profile] Atlas was rebuilt: restarting profile ${profile}`);
  const exited = new Promise((r) => child.once("exit", r));
  child.kill();
  await exited;
  // The old copy can stay locked for a moment after its process ends.
  for (let tries = 0; ; tries++) {
    try {
      start();
      break;
    } catch (e) {
      if (tries === 10) throw e;
      await sleep(500);
    }
  }
  restarting = false;
}

start();
watchFile(exe, { interval: 1000 }, (now, before) => {
  if (now.mtimeMs !== before.mtimeMs) restart();
});
