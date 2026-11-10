// Small display helpers. Plain functions: no Svelte, easy to test, usable anywhere.

import type {
  ContentKind,
  InstanceWork,
  IsoDate,
  LauncherKind,
  Loader,
  SyncProgress,
} from "$lib/types";

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

/** "Mod", "Resource pack"… */
export const KIND: Record<ContentKind, string> = {
  mod: "Mod",
  resourcepack: "Resource pack",
  shaderpack: "Shader",
  datapack: "Data pack",
};

/** "Mods", "Resource packs"…, in the order the kind filters list them. */
export const KINDS: Record<ContentKind, string> = {
  mod: "Mods",
  resourcepack: "Resource packs",
  shaderpack: "Shaders",
  datapack: "Data packs",
};

/** What an update is doing right now. */
export const SYNC_STAGES: Record<SyncProgress["stage"], string> = {
  waiting_for_game_exit: "Waiting for you to close Minecraft…",
  backup: "Backing up files…",
  download: "Downloading…",
  waiting_for_manual_downloads: "Waiting for you to download files",
  apply: "Putting files in place…",
};

/** "1 mod", "34 mods" */
export function plural(
  n: number,
  word: string,
  pluralWord = word + "s",
): string {
  return `${n} ${n === 1 ? word : pluralWord}`;
}

/** "A", "A and B", "A, B and 3 more": a long list said in one line. */
export function nameList(names: string[]): string {
  if (names.length <= 2) return names.join(" and ");
  return `${names[0]}, ${names[1]} and ${names.length - 2} more`;
}

/** "640 MB", "1.2 GB" */
export function fileSize(bytes: number): string {
  // A few bytes still show as 1 KB; nothing at all is 0 KB.
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${Math.round(bytes / 1024 ** 2)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

/** The code in a pasted friend link (".../f/7KQ29XMB" or "atlas://friend/7KQ29XMB"), or null
 *  when it isn't one: then it's a Minecraft name. */
export function friendLinkCode(input: string): string | null {
  const m = /(?:\/f\/|atlas:\/\/friend\/)([0-9a-z-]+)\/?$/i.exec(input.trim());
  const code = m?.[1].toUpperCase().replace(/-/g, "");
  return code?.length === 8 ? code : null;
}

/** "7KQ29XMB" → "7KQ2-9XMB" (easier to read out loud). */
export function formatCode(code: string): string {
  return code.length === 8 ? `${code.slice(0, 4)}-${code.slice(4)}` : code;
}

/** "Reading files… 120 of 300", "Uploading 3 of 41 files…", "Saving the new version…" */
export function workLabel(w: InstanceWork): string {
  switch (w.stage) {
    case "reading":
      return `Reading files… ${w.done.toLocaleString()} of ${w.total.toLocaleString()}`;
    case "uploading":
      // Counted in bytes, so it moves within a big file too.
      return `Uploading ${fileSize(w.done)} of ${fileSize(w.total)}…`;
    case "saving":
      return "Saving the new version…";
    case "downloading":
      return `Downloading the modpack… ${fileSize(w.done)} of ${fileSize(w.total)}`;
  }
}

/** 0–100 for a progress bar; null while there's nothing to count. */
export function workPercent(w: InstanceWork): number | null {
  return w.total ? Math.round((w.done / w.total) * 100) : null;
}

/**
 * The open button's label. Only Prism opens on the instance itself; the other launchers
 * can only be opened, so the label says just that: "Open in Prism Launcher",
 * "Open Modrinth App".
 */
export function openLabel(launcher: {
  kind: LauncherKind;
  name: string;
}): string {
  return launcher.kind === "prism"
    ? `Open in ${launcher.name}`
    : `Open ${launcher.name}`;
}
