import assert from "node:assert/strict";
import { test } from "node:test";
import { bannerPixels } from "./pixel.ts";

const rgb = (fill: string) => fill.match(/\d+/g)!.map(Number);
const light = ([r, g, b]: number[]) => (0.3 * r + 0.59 * g + 0.11 * b) / 255;

test("a group always gets the same banner", () => {
  assert.deepEqual(
    bannerPixels("0192f3a0-group"),
    bannerPixels("0192f3a0-group"),
  );
  assert.notDeepEqual(bannerPixels("one"), bannerPixels("two"));
});

test("the pattern stands out from the base", () => {
  for (let i = 0; i < 200; i++) {
    const px = bannerPixels(`group-${i}`);
    assert.equal(px.length, 64);
    const ls = px.map((p) => light(rgb(p.fill)));
    // Shading moves a pixel by 7% at most; two colours 0.28 apart still read as two.
    assert.ok(Math.max(...ls) - Math.min(...ls) > 0.2, `group-${i}`);
    // Black is only ever a pattern (32 pixels at most), never the background, which
    // would melt into Atlas's dark panels.
    assert.ok(ls.filter((l) => l < 0.22).length <= 32, `group-${i} background`);
  }
});
