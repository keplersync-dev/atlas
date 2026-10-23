// Small display helpers. Plain functions: no Svelte, easy to test, usable anywhere.

import type { InstanceWork, IsoDate, Loader } from "$lib/types";

const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

/** "just now", "35 minutes ago", "yesterday", "3 weeks ago"… */
export function timeAgo(iso: IsoDate | null, now = Date.now()): string {
  if (!iso) return "never";
  const seconds = Math.round((new Date(iso).getTime() - now) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 45) return "just now";
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["minute", 60],
    ["hour", 3600],
    ["day", 86400],
    ["week", 604800],
    ["month", 2629800],
    ["year", 31557600],
  ];
  let [unit, size] = units[0];
  for (const u of units) if (abs >= u[1]) [unit, size] = u;
  return rtf.format(Math.round(seconds / size), unit);
}

const LOADER_NAMES: Record<Loader, string> = {
  vanilla: "Vanilla",
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
};

/** "Fabric 1.21.1", "Vanilla 1.21.4", or "Unknown version" when the launcher metadata couldn't be read. */
export function versionLabel(
  loader: Loader | null,
  minecraftVersion: string | null,
): string {
  if (!loader && !minecraftVersion) return "Unknown version";
  return [loader ? LOADER_NAMES[loader] : null, minecraftVersion]
    .filter(Boolean)
    .join(" ");
}

/** "1 mod", "34 mods" */
export function plural(
  n: number,
  word: string,
  pluralWord = word + "s",
): string {
  return `${n} ${n === 1 ? word : pluralWord}`;
}

/** "640 MB", "1.2 GB" */
export function fileSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${Math.round(bytes / 1024 ** 2)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

/** Friend/group codes come in any shape: "7kq2-9xmb", "7KQ29XMB" or a full link. */
export function normalizeCode(input: string): string {
  const last = input.trim().split("/").pop() ?? "";
  return last.toUpperCase().replace(/[^0-9A-Z]/g, "");
}

/** "7KQ29XMB" → "7KQ2-9XMB" (easier to read out loud). */
export function formatCode(code: string): string {
  return code.length === 8 ? `${code.slice(0, 4)}-${code.slice(4)}` : code;
}

/** "Reading files… 120 of 300", "Uploading 3 of 41 files…", "Saving the new version…" */
export function workLabel(w: InstanceWork): string {
  switch (w.stage) {
    case "reading":
      return `Reading files… ${w.done} of ${w.total}`;
    case "uploading":
      return `Uploading ${w.done} of ${plural(w.total, "file")}…`;
    case "saving":
      return "Saving the new version…";
  }
}

/** 0–100 for a progress bar; null while there's nothing to count. */
export function workPercent(w: InstanceWork): number | null {
  return w.total ? Math.round((w.done / w.total) * 100) : null;
}

