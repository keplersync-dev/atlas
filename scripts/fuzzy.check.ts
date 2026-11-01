// Run: node --experimental-strip-types scripts/fuzzy.check.ts
import assert from "node:assert/strict";
import { fuzzy, fuzzyFilter } from "../src/lib/fuzzy.ts";

const packs = [
  "All the Mods 10 To the Sky",
  "SkyFactory 5",
  "SkyFactory 4",
  "PPTools",
];
assert.deepEqual(
  fuzzyFilter(packs, "sf5", (p) => [p]),
  ["SkyFactory 5"],
);
assert.equal(
  fuzzyFilter(packs, "atm sky", (p) => [p])[0],
  "All the Mods 10 To the Sky",
);
assert.equal(
  fuzzyFilter(packs, "sky", (p) => [p])[0],
  "SkyFactory 5",
  "word start first",
);
assert.equal(fuzzy("xyz", "PPTools"), null);
assert.equal(fuzzy("", "anything"), 0);
assert.deepEqual(
  fuzzyFilter(packs, "", (p) => [p]),
  packs,
);
console.log("fuzzy ok");
