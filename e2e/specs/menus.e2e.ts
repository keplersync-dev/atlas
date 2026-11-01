import { browser } from "@wdio/globals";
import { SIZES, atSize, capture, useProfile, writeReport } from "../helpers";

// Right-click and ⋯ menus with real mouse input: each one opens, has items, and fits in
// the window, at every size. Read-only: menus are closed with Escape.
const PROFILES = (process.env.ATLAS_PROFILES ?? "haloxx,dakep").split(",");

const go = (view: object) =>
  browser.execute((v) => (window as any).__atlas.go(v), view);

type Target = { name: string; page: object; selector: string };

const TARGETS: Target[] = [
  {
    name: "friend-row",
    page: { page: "friends" },
    selector: "main section:first-of-type .group",
  },
  {
    name: "group-panel",
    page: { page: "friends" },
    selector: "main section:last-of-type .grid.gap-3 > div > div:first-child",
  },
  {
    name: "group-member",
    page: { page: "friends" },
    selector: "main section:last-of-type .grid.gap-3 .group",
  },
  { name: "home-card", page: { page: "home" }, selector: "main article" },
  // The friends column only shows from 1180 px.
  {
    name: "home-friend",
    page: { page: "home" },
    selector: "main aside .group",
  },
  {
    name: "sidebar-instance",
    page: { page: "home" },
    selector: "button[title][class*='min-h-10']",
  },
  {
    name: "shared-row",
    page: { page: "shared" },
    selector: "main section div[class*='gap-3.5 px-4 py-2.5']",
  },
];

/** The open menu: how many items, and whether it's inside the window. */
const menuState = () =>
  browser.execute(() => {
    const m = document.querySelector("[role=menu]");
    if (!m) return null;
    const r = m.getBoundingClientRect();
    return {
      items: [...m.querySelectorAll("[role=menuitem]")].map((b) =>
        b.textContent!.trim().replace(/\s+/g, " "),
      ),
      inside:
        r.left >= 0 &&
        r.top >= 0 &&
        r.right <= innerWidth &&
        r.bottom <= innerHeight,
    };
  });

describe("menus", () => {
  after(() => writeReport());
  for (const profile of PROFILES)
    it(`open everywhere as ${profile}`, async () => {
      await useProfile(profile);
      const notes: string[] = [];
      try {
        for (const t of TARGETS)
          for (const size of SIZES) {
            await atSize(size);
            await go(t.page);
            await browser.pause(500);
            const els = await browser.$$(t.selector).getElements();
            if (!els.length) {
              notes.push(`${t.name} ${size.name}: nothing to right-click here`);
              continue;
            }
            // The first one that's showing: someone else, not you, in a group.
            let el: (typeof els)[number] | undefined;
            for (const e of els)
              if (await e.isDisplayed()) {
                el = e;
                break;
              }
            if (!el) {
              notes.push(`${t.name} ${size.name}: hidden at this size`);
              continue;
            }
            await el.scrollIntoView({ block: "center" });
            await el.click({ button: "right" });
            await browser.pause(250);
            const m = await menuState();
            if (!m) notes.push(`${t.name} ${size.name}: no menu`);
            else {
              if (!m.inside)
                notes.push(`${t.name} ${size.name}: menu outside the window`);
              if (size.name === "mid")
                notes.push(`${t.name}: ${m.items.join(" | ")}`);
              await capture(profile, `menu-${t.name}`, size);
            }
            await browser.keys("Escape");
            await browser.pause(150);
          }
      } finally {
        await atSize(null);
        await go({ page: "home" });
        console.log(`\n${profile}\n  ${notes.join("\n  ")}`);
      }
    });
});
