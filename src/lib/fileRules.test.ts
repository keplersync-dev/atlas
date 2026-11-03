// The same cases as the tests in src-tauri/src/services/share_files.rs: the two have to
// agree on what's shared. Run with `npm test`.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { FileRules } from "./types";
import { isShared, keptAs } from "./fileRules.ts";

const rules = (include: string[], exclude: string[]): FileRules => ({
  include,
  exclude,
  firstInstall: [],
});

test("defaults tick configs, scripts and the pack's own files", () => {
  const none = rules([], []);
  assert.ok(isShared(none, "config/create/server.toml", false));
  assert.ok(isShared(none, "scripts/recipes.zs", false));
  assert.ok(
    isShared(none, "resources/test/textures/gui/title/background/bg.png", false),
  );
  assert.ok(isShared(none, "global_packs/a/pack.mcmeta", true), "from the pack");
  assert.ok(!isShared(none, "journeymap/data.json", false));
  assert.ok(!isShared(none, "options.txt", false));
});

test("the longest rule wins", () => {
  const r = rules(
    ["config/create/server.toml", "journeymap/"],
    ["config/create/"],
  );
  assert.ok(!isShared(r, "config/create/client.toml", false));
  assert.ok(isShared(r, "config/create/server.toml", false), "file beats folder");
  assert.ok(isShared(r, "journeymap/data.json", false));
  assert.ok(isShared(r, "config/sodium.json", false), "default still applies");
  const allOff = rules([], [""]);
  assert.ok(!isShared(allOff, "config/sodium.json", false));
  assert.ok(!isShared(allOff, "global_packs/a.json", true));
  const allOn = rules([""], []);
  assert.ok(isShared(allOn, "journeymap/data.json", false));
});

test("personal files are kept as Modrinth keeps them", () => {
  const none = rules([], []);
  assert.equal(keptAs(none, "options.txt"), "first_install");
  assert.equal(keptAs(none, "servers.dat"), "merged");
  assert.equal(keptAs(none, "config/a.json"), null);
  const mine = { ...none, firstInstall: ["optionsof.txt"] };
  assert.equal(keptAs(mine, "optionsof.txt"), "first_install");
});
