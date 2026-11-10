import assert from "node:assert/strict";
import { test } from "node:test";
import { friendLinkCode } from "./format.ts";

test("friend links give their code; anything else is a Minecraft name", () => {
  assert.equal(friendLinkCode("https://keplersync.app/f/7KQ29XMB"), "7KQ29XMB");
  assert.equal(friendLinkCode(" keplersync.app/f/7kq2-9xmb/ "), "7KQ29XMB");
  assert.equal(friendLinkCode("atlas://friend/7KQ29XMB"), "7KQ29XMB");
  assert.equal(friendLinkCode("7KQ29XMB"), null, "a bare code could be a name");
  assert.equal(friendLinkCode("Notch"), null);
  assert.equal(friendLinkCode("https://keplersync.app/f/short"), null);
});
