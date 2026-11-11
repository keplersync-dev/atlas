import { isShared, keptAs } from "./fileRules";
import type {
  FileRules,
  Id,
  PublishItem,
  PublishPlan,
  ShareFile,
  ShareMode,
} from "./types";

/** The setting picked in the dialog, or the item's saved one. */
export const modeOf = (item: PublishItem, modes: Record<Id, ShareMode>) =>
  modes[item.contentId] ?? item.shareMode;

/** Whether two sets of file ticks share the same `files`, kept the same way. */
export function sameRules(a: FileRules, b: FileRules, files: ShareFile[]) {
  return files.every((f) => {
    const shared = isShared(a, f.path, f.fromPack, f.personal);
    return (
      shared === isShared(b, f.path, f.fromPack, f.personal) &&
      (!shared ||
        keptAs(a, f.path, f.personal) === keptAs(b, f.path, f.personal))
    );
  });
}

/**
 * What the next version would change compared with the shared one, with the dialog's
 * settings (`modes`) and ticks (`rules`, null while untouched). Nothing here means there's
 * nothing to send.
 */
export function publishChanges(
  plan: PublishPlan,
  modes: Record<Id, ShareMode>,
  rules: FileRules | null,
) {
  const added: PublishItem[] = [];
  const changed: PublishItem[] = [];
  const updated: PublishItem[] = [];
  for (const i of plan.items) {
    const mode = modeOf(i, modes);
    if (i.publishedShareMode === null || i.publishedShareMode === "private") {
      if (mode !== "private") added.push(i);
    } else if (mode !== i.publishedShareMode) {
      changed.push(i);
    } else if (i.contentsChanged) {
      updated.push(i);
    }
  }
  const ticks = !!rules && !sameRules(rules, plan.rules, plan.files);
  const total =
    added.length +
    changed.length +
    updated.length +
    plan.removed.length +
    plan.filesChanged.length +
    (ticks ? 1 : 0) +
    (plan.versions ? 1 : 0);
  return { added, changed, updated, ticks, total };
}
