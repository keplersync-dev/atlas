<script lang="ts" module>
  import { openDialog } from "$lib/dialogs.svelte";
  import { plural, timeAgo } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { instances } from "$lib/stores";
  import type { InstanceSummary, SharedInstance } from "$lib/types";

  export type Tone = "accent" | "warn" | "online" | "muted";
  export type RowState = {
    tone: Tone;
    text: string;
    action?: { label: string; variant: "primary" | "warn"; run: () => void };
    /** Counts toward "needs attention" badges (Shared page, friend dropdowns). */
    attention?: "new" | "update" | "changes";
  };

  export const localFor = (s: SharedInstance): InstanceSummary | undefined =>
    s.localInstanceId ? instances.byId(s.localInstanceId) : undefined;

  /** One plain-language line per instance, plus a button only when there's something to do. */
  export function rowState(s: SharedInstance): RowState {
    const local = localFor(s);
    if (s.myRole === "owner") {
      const sync = local?.sync;
      if (sync?.state === "local_changes")
        return {
          tone: "accent",
          text: `${plural(sync.localChangeCount, "change")} not shared yet`,
          action: {
            label: "Publish",
            variant: "primary",
            run: () => openDialog({ kind: "publish", instanceId: local!.id }),
          },
          attention: "changes",
        };
      const others = s.members.filter((m) => m.role !== "owner");
      const active = others.filter((m) => m.status === "active");
      const behind = active.filter(
        (m) => (m.lastSyncedRevision ?? 0) < s.latestRevision,
      ).length;
      const invited = others.filter((m) => m.status === "invited").length;
      const parts = [
        active.length
          ? `Shared with ${plural(active.length, "person", "people")}`
          : "Nobody has added it yet",
      ];
      if (behind) parts.push(`${behind} not updated yet`);
      else if (active.length) parts.push("everyone up to date");
      if (invited) parts.push(`${invited} invited`);
      return { tone: "muted", text: parts.join(" · ") };
    }
    if (s.myStatus === "invited")
      return {
        tone: "accent",
        text: `New${s.myViaGroup ? ` · to ${s.myViaGroup.name}` : ""} · ${timeAgo(s.invitedAt)}`,
        action: {
          label: "Add",
          variant: "primary",
          run: () => openDialog({ kind: "accept", sharedInstanceId: s.id }),
        },
        attention: "new",
      };
    if (s.frozen)
      return {
        tone: "muted",
        text: "Frozen · the owner left, no more updates",
      };
    if (!local) return { tone: "muted", text: "Not on this PC" };
    if (local.isRunning) return { tone: "online", text: "Playing now" };
    if (local.sync?.state === "update_available")
      return {
        tone: "warn",
        text: "Update ready",
        action: {
          label: "Update",
          variant: "warn",
          run: () => go({ page: "review", id: local.id }),
        },
        attention: "update",
      };
    if (local.sync?.state === "conflict")
      return {
        tone: "warn",
        text: "Update ready · would undo your changes",
        action: {
          label: "Review",
          variant: "warn",
          run: () => go({ page: "review", id: local.id }),
        },
        attention: "update",
      };
    if (local.sync?.state === "local_changes")
      return {
        tone: "muted",
        text: `${plural(local.sync.localChangeCount, "local change")} · ${local.launcher.name}`,
      };
    return { tone: "muted", text: `Up to date · ${local.launcher.name}` };
  }

  /** Attention counts for a list of shared instances, for header badges. */
  export function attentionOf(list: SharedInstance[]) {
    const counts = { new: 0, update: 0, changes: 0, total: 0 };
    for (const s of list) {
      const a = rowState(s).attention;
      if (a) {
        counts[a]++;
        counts.total++;
      }
    }
    return counts;
  }

  /** Needs-you first, then alphabetical. */
  export const byAttention = (a: SharedInstance, b: SharedInstance) =>
    Number(!!rowState(b).attention) - Number(!!rowState(a).attention) ||
    a.name.localeCompare(b.name);
</script>

<script lang="ts">
  import { fileSize, versionLabel } from "$lib/format";
  import { social } from "$lib/stores";
  import Button from "./Button.svelte";
  import ContentIcon from "./ContentIcon.svelte";
  import Icon from "./Icon.svelte";

  // One shared instance as a list row: name, plain status, and the one action that matters.
  // Used on the Shared page and in each friend's dropdown on the Friends page.

  let {
    s,
    /** Show who shared it (the Shared page). Off inside a friend's dropdown, where it's obvious. */
    showOwner = false,
  }: { s: SharedInstance; showOwner?: boolean } = $props();

  const st = $derived(rowState(s));
  const local = $derived(localFor(s));
  const invited = $derived(s.myStatus === "invited");

  let busy = $state(false);
  let error = $state<string | null>(null);
  async function decline() {
    busy = true;
    error = null;
    try {
      await social.declineInvite(s.id);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  const TONE: Record<Tone, string> = {
    accent: "text-(--accent)",
    warn: "text-(--warn)",
    online: "text-(--online)",
    muted: "text-(--text-muted)",
  };
  const DOT: Record<Tone, string> = {
    accent: "bg-(--accent)",
    warn: "bg-(--warn)",
    online: "bg-(--online)",
    muted: "bg-(--text-faint)",
  };
</script>

<div
  class="flex items-center gap-3.5 px-4 py-2.5 not-last:border-b not-last:border-(--line)"
>
  <!-- Click: open it if it's on this PC, or start adding it if it's a new invite.
       Otherwise there's nothing to open, so it's plain text (not a disabled button,
       which would look faded). -->
  <svelte:element
    this={local || invited ? "button" : "div"}
    role={local || invited ? undefined : "presentation"}
    onclick={() =>
      local
        ? go({ page: "instance", id: local.id })
        : invited && openDialog({ kind: "accept", sharedInstanceId: s.id })}
    class="flex min-w-0 flex-1 items-center gap-3.5 rounded-md text-left"
  >
    <ContentIcon
      src={local?.icon ?? s.icon}
      seed={s.localInstanceId ?? s.id}
      class="size-10 rounded-lg"
    />
    <span class="min-w-0 flex-1">
      <b class="block truncate font-semibold">{s.name}</b>
      <small class="block truncate text-xs text-(--text-faint)">
        {#if showOwner}
          <span class="text-(--text-muted)"
            >{s.owner ? `From ${s.owner.displayName}` : "No owner"}</span
          > ·
        {/if}
        {versionLabel(s.loader, s.minecraftVersion)} · {plural(
          s.contentCount,
          "item",
        )}
        {#if s.optionalItems.length}· {s.optionalItems.length} optional{/if}
        {#if invited}· {fileSize(s.sizeBytes)}{/if}
        {#if s.description}· {s.description}{/if}
      </small>
    </span>
  </svelte:element>

  <span class="flex shrink-0 flex-col items-end">
    <span class="flex items-center gap-1.5 text-xs {TONE[st.tone]}">
      <i class="size-1.5 rounded-full {DOT[st.tone]}"></i>{st.text}
    </span>
    {#if error}<small class="text-[11px] text-(--danger)" role="alert"
        >{error}</small
      >{/if}
  </span>

  <span class="flex w-30 shrink-0 items-center justify-end gap-1">
    {#if st.action}
      <Button size="sm" variant={st.action.variant} onclick={st.action.run}
        >{st.action.label}</Button
      >
    {/if}
    {#if invited}
      <button
        onclick={decline}
        disabled={busy}
        title="Decline"
        aria-label="Decline {s.name}"
        class="grid size-7 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--text-primary) disabled:opacity-40"
      >
        <Icon name="close" class="size-3.5" />
      </button>
    {/if}
  </span>
</div>
