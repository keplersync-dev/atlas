// The same cases as the tests in src-tauri/src/services/share_files.rs: the two have to
// agree on what's shared. Run with `npm test`.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { FileRules } from "./types";
import { isShared, keepOf, keptAs, setKeep } from "./fileRules.ts";

const rules = (include: string[], exclude: string[]): FileRules => ({
  include,
  exclude,
  firstInstall: [],
  everyUpdate: [],
});

test("defaults tick configs, scripts and the pack's own files", () => {
  const none = rules([], []);
  assert.ok(isShared(none, "config/create/server.toml", false));
  assert.ok(isShared(none, "scripts/recipes.zs", false));
  assert.ok(
    isShared(
      none,
      "resources/test/textures/gui/title/background/bg.png",
      false,
    ),
  );
  assert.ok(
    isShared(none, "global_packs/a/pack.mcmeta", true),
    "from the pack",
  );
  assert.ok(!isShared(none, "journeymap/data.json", false));
  assert.ok(!isShared(none, "options.txt", false));
});

test("the longest rule wins", () => {
  const r = rules(
    ["config/create/server.toml", "journeymap/"],
    ["config/create/"],
  );
  assert.ok(!isShared(r, "config/create/client.toml", false));
  assert.ok(
    isShared(r, "config/create/server.toml", false),
    "file beats folder",
  );
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

// The same cases as personal_files_get_their_own_defaults in share_files.rs.
const jei = {
  keep: "only_me" as const,
  what: "JEI bookmarks",
  rule: "config/jei/bookmarks.ini",
};
const iris = {
  keep: "once" as const,
  what: "shader choice",
  rule: "config/iris.properties",
};
const file = (path: string, personal: typeof jei | typeof iris | null) => ({
  path,
  sizeBytes: 1,
  modified: null,
  fromPack: false,
  changed: false,
  refusal: null,
  personal,
});

test("personal files get their own defaults", () => {
  const none = rules([], []);
  assert.equal(keptAs(none, "config/iris.properties", iris), "first_install");
  assert.ok(!isShared(none, "config/jei/bookmarks.ini", false, jei), "only me");
  assert.ok(
    isShared(none, "config/jei/bookmarks.ini", true, jei),
    "a pack's own copy",
  );
  const folder = rules(["config/jei/"], []);
  assert.ok(!isShared(folder, "config/jei/bookmarks.ini", false, jei));
  const named = rules(["config/jei/bookmarks.ini"], []);
  assert.ok(isShared(named, "config/jei/bookmarks.ini", false, jei));
});

test("the switch sets and reads back each choice", () => {
  const none = rules([], []);
  const cases = [
    file("config/iris.properties", iris),
    file("config/jei/bookmarks.ini", jei),
    file("config/create-client.toml", null),
  ];
  for (const f of cases)
    for (const to of ["always", "once", "me"] as const) {
      const r = setKeep(none, f, to);
      assert.equal(keepOf(r, f), to, `${f.path} → ${to}`);
      assert.equal(keepOf(setKeep(r, f, keepOf(none, f)), f), keepOf(none, f));
    }
  assert.deepEqual(setKeep(none, cases[0], "always").everyUpdate, [
    "config/iris.properties",
  ]);
  assert.deepEqual(
    setKeep(none, cases[0], "once"),
    none,
    "once is already its default",
  );
});
