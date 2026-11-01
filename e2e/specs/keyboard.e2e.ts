import { browser } from "@wdio/globals";
import { writeFileSync, mkdirSync } from "node:fs";
import { OUT, SIZES, atSize, useProfile } from "../helpers";

// Tabs through each page and dialog with real key presses, at the middle size. Reports
// controls that show no focus, focus that leaves an open dialog, and dialogs Escape
// doesn't close. Read-only, like the walk.
const PROFILE = process.env.ATLAS_PROFILES?.split(",")[0] ?? "haloxx";
const MAX_TABS = 80;

type Where = { name: string; open: () => Promise<unknown>; dialog?: boolean };

const go = (view: object) =>
  browser.execute((v) => (window as any).__atlas.go(v), view);
const dialog = (d: object) =>
  browser.execute((v) => (window as any).__atlas.openDialog(v), d);

/** The focused element: a short name, whether focus shows, and whether it's in view. */
const focused = () =>
  browser.execute(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return null;
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const ring =
      (cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0) ||
      cs.boxShadow !== "none";
    // Text fields and some controls show focus with an accent border instead of a ring.
    const probe = document.createElement("i");
    probe.style.color = "var(--accent)";
    document.body.append(probe);
    const accent = getComputedStyle(probe).color;
    probe.remove();
    const bordered =
      cs.borderTopColor === accent && parseFloat(cs.borderTopWidth) > 0;
    const field = el.matches(
      "input:not([type=checkbox]):not([type=radio]), textarea",
    );
    let seen = Number(cs.opacity) > 0.05;
    for (let p = el.parentElement; p && seen; p = p.parentElement)
      seen = Number(getComputedStyle(p).opacity) > 0.05;
    const dialogs = [...document.querySelectorAll("dialog[open]")];
    return {
      name: (
        el.getAttribute("aria-label") ||
        el.textContent ||
        (el as HTMLInputElement).placeholder ||
        el.tagName
      )
        .trim()
        .replace(/\s+/g, " ")
        .slice(0, 60),
      tag: el.tagName,
      shows: ring || field || bordered,
      visible: seen && r.width > 0 && r.height > 0,
      inDialog: dialogs.length
        ? dialogs[dialogs.length - 1].contains(el)
        : null,
    };
  });

describe("keyboard", () => {
  it(`works through every page as ${PROFILE}`, async () => {
    await useProfile(PROFILE);
    await atSize(SIZES[1]);
    const ids: string[] = await browser.execute(() =>
      (window as any).__atlas.instances.list.map((i: any) => i.id),
    );
    const places: Where[] = [
      { name: "home", open: () => go({ page: "home" }) },
      { name: "shared", open: () => go({ page: "shared" }) },
      { name: "friends", open: () => go({ page: "friends" }) },
      ...(ids[0]
        ? [
            {
              name: "instance",
              open: () => go({ page: "instance", id: ids[0] }),
            },
          ]
        : []),
      {
        name: "settings",
        open: () => dialog({ kind: "account" }),
        dialog: true,
      },
      {
        name: "add instances",
        open: () => dialog({ kind: "track" }),
        dialog: true,
      },
      {
        name: "uploads",
        open: () => dialog({ kind: "storage" }),
        dialog: true,
      },
    ];
    const report: Record<string, string[]> = {};
    try {
      for (const place of places) {
        const notes: string[] = [];
        await place.open();
        await browser.pause(600);
        if (!place.dialog)
          await browser.execute(() =>
            (document.activeElement as HTMLElement)?.blur(),
          );
        const order: string[] = [];
        for (let i = 0; i < MAX_TABS; i++) {
          await browser.keys("Tab");
          let f = await focused();
          // A border that fades in (transition-colors) needs a moment before it shows.
          if (f && !f.shows) {
            await browser.pause(250);
            f = await focused();
          }
          if (!f) {
            // The end of a page's order. In a dialog, Chromium passes through the page
            // once and the next Tab must land back inside.
            if (place.dialog) {
              await browser.keys("Tab");
              if ((await focused())?.inDialog !== true)
                notes.push(`focus left the dialog after ${order.length} stops`);
            }
            break;
          }
          const key = `${f.tag} ${f.name}`;
          if (order[0] === key && order.length > 1) break; // came round again
          order.push(key);
          if (!f.shows) notes.push(`no visible focus: ${key}`);
          if (!f.visible) notes.push(`focused but invisible: ${key}`);
          if (f.inDialog === false) notes.push(`focus left the dialog: ${key}`);
        }
        if (place.dialog) {
          await browser.keys("Escape");
          await browser.pause(300);
          const open = await browser.execute(
            () => document.querySelectorAll("dialog[open]").length,
          );
          if (open) {
            notes.push("Escape didn't close it");
            await browser.execute(() => (window as any).__atlas.closeDialog());
          }
        }
        report[place.name] = [
          `${order.length} stops: ${order.join(" → ")}`,
          ...notes,
        ];
      }
    } finally {
      await browser.execute(() => (window as any).__atlas.closeDialog());
      await atSize(null);
      await go({ page: "home" });
      mkdirSync(OUT, { recursive: true });
      writeFileSync(
        new URL("keyboard.json", OUT),
        JSON.stringify(report, null, 2),
      );
      for (const [where, notes] of Object.entries(report))
        console.log(`\n${where}\n  ${notes.join("\n  ")}`);
    }
  });
});
