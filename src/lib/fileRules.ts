// The share dialog's file tree: which files are ticked, and the folders they sit in.
// The rules mirror src-tauri/src/services/share_files.rs, which decides for real when
// sharing; keep the two the same.

import type { FileRules, ShareFile } from "$lib/types";

/** Ticked when no rule says otherwise. DEFAULT_DIRS in share_files.rs. */
const DEFAULT_DIRS = ["config/", "defaultconfigs/", "kubejs/", "scripts/"];
/** Always first install only. FIRST_INSTALL in share_files.rs. */
const FIRST_INSTALL = ["options.txt"];

/** A rule covers itself, and a folder rule ("dir/", or "" for all) what's inside. */
function covers(rule: string, path: string) {
  return (
    rule === path ||
    rule === "" ||
    (rule.endsWith("/") && path.startsWith(rule))
  );
}

function longest(rules: string[], path: string) {
  let best = -1;
  for (const r of rules)
    if (covers(r, path) && r.length > best) best = r.length;
  return best;
}

/** Whether a file is ticked. `fromPack`: the modpack ships it. */
export function isShared(rules: FileRules, path: string, fromPack: boolean) {
  const inc = longest(rules.include, path);
  const exc = longest(rules.exclude, path);
  if (inc < 0 && exc < 0)
    return fromPack || DEFAULT_DIRS.some((d) => path.startsWith(d));
  return inc > exc;
}

/** Tick or untick a file or folder: the rules inside it go, and one rule says it. */
export function setShared(
  rules: FileRules,
  target: string,
  on: boolean,
): FileRules {
  const outside = (r: string) => !covers(target, r);
  return {
    ...rules,
    include: [...rules.include.filter(outside), ...(on ? [target] : [])],
    exclude: [...rules.exclude.filter(outside), ...(on ? [] : [target])],
  };
}

/** How members' copies are kept. `keep` in share_files.rs. */
export function keptAs(rules: FileRules, path: string) {
  if (path === "servers.dat") return "merged" as const;
  if (FIRST_INSTALL.includes(path) || rules.firstInstall.includes(path))
    return "first_install" as const;
  return null;
}

/** Whether the owner can switch this file to first install only (options.txt always is). */
export const canChooseFirstInstall = (path: string) =>
  path !== "servers.dat" && !FIRST_INSTALL.includes(path);

export function setFirstInstall(
  rules: FileRules,
  path: string,
  on: boolean,
): FileRules {
  const rest = rules.firstInstall.filter((p) => p !== path);
  return { ...rules, firstInstall: on ? [...rest, path] : rest };
}

/** A file that can't be uploaded and doesn't come from the pack never goes out. */
export const blocked = (f: ShareFile) => !!f.refusal && !f.fromPack;

/** The defaults: config and script folders and the pack's own files, nothing else. */
export const DEFAULT_RULES: FileRules = {
  include: [],
  exclude: [],
  firstInstall: [],
};

export interface Folder {
  name: string;
  /** "config/create/" */
  path: string;
  folders: Folder[];
  files: ShareFile[];
  /** Everything inside, and the newest change inside. */
  sizeBytes: number;
  modified: string | null;
}

/** The tree of `files`. Levels aren't sorted: the table sorts what it shows. */
export function buildTree(files: ShareFile[]): Folder {
  const folder = (name: string, path: string): Folder => ({
    name,
    path,
    folders: [],
    files: [],
    sizeBytes: 0,
    modified: null,
  });
  const root = folder("", "");
  const index = new Map<string, Folder>([["", root]]);
  for (const f of files) {
    const parts = f.path.split("/");
    let parent = root;
    const add = (to: Folder) => {
      to.sizeBytes += f.sizeBytes;
      if (f.modified && (!to.modified || f.modified > to.modified))
        to.modified = f.modified;
    };
    add(root);
    for (let i = 0; i < parts.length - 1; i++) {
      const path = parts.slice(0, i + 1).join("/") + "/";
      let next = index.get(path);
      if (!next) {
        next = folder(parts[i], path);
        index.set(path, next);
        parent.folders.push(next);
      }
      add(next);
      parent = next;
    }
    parent.files.push(f);
  }
  return root;
}

/** The folder at `path` ("" for the top). */
export function folderAt(root: Folder, path: string): Folder {
  let at = root;
  for (const part of path.split("/").filter(Boolean)) {
    const next = at.folders.find((f) => f.name === part);
    if (!next) return root;
    at = next;
  }
  return at;
}

export interface Count {
  /** Files that can be shared. */
  total: number;
  shared: number;
  sharedBytes: number;
}

/** Ticked files per folder path ("" for all), for partly ticked folders and the summary. */
export function countShared(root: Folder, rules: FileRules) {
  const counts = new Map<string, Count>();
  const walk = (folder: Folder): Count => {
    const c: Count = { total: 0, shared: 0, sharedBytes: 0 };
    for (const f of folder.files) {
      if (blocked(f)) continue;
      c.total++;
      if (isShared(rules, f.path, f.fromPack)) {
        c.shared++;
        c.sharedBytes += f.sizeBytes;
      }
    }
    for (const sub of folder.folders) {
      const s = walk(sub);
      c.total += s.total;
      c.shared += s.shared;
      c.sharedBytes += s.sharedBytes;
    }
    counts.set(folder.path, c);
    return c;
  };
  walk(root);
  return counts;
}
