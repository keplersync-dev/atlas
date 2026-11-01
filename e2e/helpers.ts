import { browser } from "@wdio/globals";
import { mkdirSync, writeFileSync } from "node:fs";

/** The three sizes every screen is checked at: the minimum window, a middle size, and
 *  full screen on a 2560 px display at 125%. */
export const SIZES = [
  { name: "min", width: 1051, height: 651 },
  { name: "mid", width: 1440, height: 860 },
  { name: "full", width: 2048, height: 1110 },
] as const;
export type Size = (typeof SIZES)[number];

export const OUT = new URL("./out/", import.meta.url);

/** A Chrome DevTools command through the driver's session (Edge's CDP passthrough). */
export async function cdp(cmd: string, params: object = {}) {
  const { protocol = "http", hostname = "localhost", port } = browser.options;
  const res = await fetch(
    `${protocol}://${hostname}:${port}/session/${browser.sessionId}/ms/cdp/execute`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ cmd, params }),
    },
  );
  const body = (await res.json()) as { value: unknown };
  if (!res.ok) throw new Error(`${cmd}: ${JSON.stringify(body.value)}`);
  return body.value;
}

/** Lay the page out at this size without moving the real window. */
export async function atSize(size: Size | null) {
  if (size)
    await cdp("Emulation.setDeviceMetricsOverride", {
      width: size.width,
      height: size.height,
      deviceScaleFactor: 1,
      mobile: false,
    });
  else await cdp("Emulation.clearDeviceMetricsOverride");
  await browser.pause(250);
}

/** Switch to the Atlas window signed in as `username` (shown at the sidebar's foot). */
export async function useProfile(username: string) {
  for (const handle of await browser.getWindowHandles()) {
    await browser.switchToWindow(handle);
    const who = await browser.execute(
      () => (window as any).__atlas?.social?.session?.user?.username ?? "",
    );
    if (who.toLowerCase() === username.toLowerCase()) return;
  }
  throw new Error(`no Atlas window signed in as ${username}`);
}

export interface Finding {
  kind: string;
  detail: string;
}

/** Layout and accessibility checks in the page: sideways scrolling, things past the
 *  window's edge, text cut off with no way to read it, small text, controls with no
 *  name, small click targets. Scoped to the open dialog when there is one. */
export function audit(): Promise<Finding[]> {
  return browser.execute(() => {
    const out: { kind: string; detail: string }[] = [];
    const say = (el: Element) =>
      (el.getAttribute("aria-label") || el.textContent || "")
        .trim()
        .replace(/\s+/g, " ")
        .slice(0, 70);
    const shown = (el: Element) => {
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      return (
        r.width > 0 &&
        r.height > 0 &&
        cs.visibility !== "hidden" &&
        Number(cs.opacity) > 0.05
      );
    };
    const se = document.scrollingElement!;
    if (se.scrollWidth > se.clientWidth + 1)
      out.push({
        kind: "page scrolls sideways",
        detail: `${se.scrollWidth} > ${se.clientWidth}`,
      });
    const root =
      [...document.querySelectorAll("dialog[open]")].pop() ?? document.body;
    for (const el of root.querySelectorAll("*")) {
      if (!shown(el) || el.closest("svg")) continue;
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      // Inside something that scrolls sideways on purpose, the edge doesn't matter.
      const scroller = (() => {
        for (let p = el.parentElement; p; p = p.parentElement) {
          const o = getComputedStyle(p).overflowX;
          if (o === "auto" || o === "scroll" || o === "hidden") return p;
        }
        return null;
      })();
      if (r.right > innerWidth + 1 && !scroller)
        out.push({
          kind: "past the right edge",
          detail: `${el.tagName} ${say(el)}`,
        });
      const text = [...el.childNodes].some(
        (n) => n.nodeType === 3 && n.textContent!.trim(),
      );
      if (
        text &&
        el.scrollWidth > el.clientWidth + 1 &&
        (cs.textOverflow === "ellipsis" || cs.overflowX === "hidden") &&
        !el.closest("[title]")
      )
        out.push({ kind: "cut off, no tooltip", detail: say(el) });
      if (text && parseFloat(cs.fontSize) < 13)
        out.push({
          kind: "text under 13px",
          detail: `${cs.fontSize} ${say(el)}`,
        });
      if (
        el.matches(
          "button, a[href], [role=button], [role=menuitem], [role=tab], [role=radio], input, select, textarea",
        )
      ) {
        const input = el as HTMLInputElement;
        const name =
          el.getAttribute("aria-label") ||
          el.getAttribute("title") ||
          el.textContent!.trim() ||
          el.getAttribute("placeholder") ||
          input.labels?.[0]?.textContent?.trim() ||
          el.closest("label")?.textContent?.trim();
        if (!name)
          out.push({
            kind: "control with no name",
            detail: el.outerHTML.slice(0, 140),
          });
        const box = el.matches("input[type=checkbox], input[type=radio]")
          ? (el.closest("label")?.getBoundingClientRect() ?? r)
          : r;
        if (box.width < 24 || box.height < 24)
          out.push({
            kind: "click target under 24px",
            detail: `${Math.round(box.width)}x${Math.round(box.height)} ${say(el) || el.tagName}`,
          });
      }
    }
    const seen = new Set<string>();
    return out.filter((f) => {
      const k = f.kind + f.detail;
      return !seen.has(k) && seen.add(k);
    });
  });
}

const report: Record<string, Finding[]> = {};

/** Check and photograph the current state at one size. */
export async function capture(profile: string, step: string, size: Size) {
  const dir = new URL(`${profile}/`, OUT);
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(
    new URL(`${step}-${size.name}.png`, dir).pathname.slice(1),
  );
  const findings = await audit();
  if (findings.length) report[`${profile} ${step} ${size.name}`] = findings;
}

export function writeReport() {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(new URL("report.json", OUT), JSON.stringify(report, null, 2));
  return report;
}
