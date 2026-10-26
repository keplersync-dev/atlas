<script lang="ts" module>
  import type { ActivityItem, ActivityType } from "$lib/types";

  /** Activity that's about shared instances (the Shared page leaves out friend requests etc.). */
  export const SHARING_TYPES = new Set<ActivityType>([
    "instance_updated",
    "instance_invite",
    "instance_shared_to_group",
    "member_joined",
    "member_left",
    "ownership_transferred",
    "instance_frozen",
    "role_changed",
  ]);

  /** One event as a sentence in three parts, so the names can be bold. `me` is the
   * signed-in user's id. */
  function describe(
    a: ActivityItem,
    me: string | undefined,
  ): {
    who: string | null;
    verb: string;
    what: string;
    tail: string;
  } {
    const who = a.actor?.displayName ?? null;
    const what = a.sharedInstanceName ?? "an instance";
    switch (a.type) {
      case "instance_updated":
        return {
          who,
          verb: a.revision ? `published v${a.revision} of` : "updated",
          what,
          tail: "",
        };
      case "instance_invite":
        return { who, verb: "shared", what, tail: " with you" };
      case "instance_shared_to_group":
        return {
          who,
          verb: "shared",
          what,
          tail: ` with ${a.groupName ?? "a group"}`,
        };
      case "member_joined":
        return { who, verb: "added", what, tail: "" };
      case "member_left":
        return { who, verb: "left", what, tail: "" };
      case "ownership_transferred":
        return a.actor && a.actor.id === me
          ? { who: "You", verb: "now own", what, tail: "" }
          : { who, verb: "now owns", what, tail: "" };
      case "instance_frozen":
        return {
          who: null,
          verb: "",
          what,
          tail: " was frozen: the owner left",
        };
      case "role_changed":
        return {
          who,
          verb: `made you ${a.role === "editor" ? "an editor" : a.role === "owner" ? "the owner" : "a viewer"} of`,
          what,
          tail: "",
        };
      default:
        return { who, verb: "", what, tail: "" };
    }
  }
</script>

<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { timeAgo } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { social } from "$lib/stores";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";

  // Recent events about shared instances, one per line.
  let { items }: { items: ActivityItem[] } = $props();

  /** Where clicking an event takes you: the instance if it's on this PC, the invite if it's new. */
  function target(a: ActivityItem): (() => void) | null {
    const s = social.shared.find((x) => x.id === a.sharedInstanceId);
    if (!s) return null;
    if (s.localInstanceId) {
      const id = s.localInstanceId;
      return () => go({ page: "instance", id });
    }
    if (s.myStatus === "invited")
      return () => openDialog({ kind: "accept", sharedInstanceId: s.id });
    return null;
  }
</script>

{#snippet sentence(a: ActivityItem)}
  {@const d = describe(a, social.session?.user?.id)}
  {#if d.who}<b class="font-semibold">{d.who}</b>{/if}
  {d.verb}
  <b class="font-semibold">{d.what}</b>{d.tail}
{/snippet}

{#snippet avatar(a: ActivityItem, size: number)}
  {#if a.actor}
    <Head user={a.actor} {size} ring="var(--bg-raise)" />
  {:else}
    <span
      class="grid shrink-0 place-items-center rounded-full bg-(--bg-raise-2) text-(--text-faint)"
      style="width: {size}px; height: {size}px"
    >
      <Icon name="warn" class="size-3" />
    </span>
  {/if}
{/snippet}

{#if items.length === 0}
  <p class="text-[12.5px] text-(--text-muted)">Nothing yet.</p>
{:else}
  <div class="flex flex-col">
    {#each items as a (a.id)}
      {@const go_ = target(a)}
      <svelte:element
        this={go_ ? "button" : "div"}
        role={go_ ? undefined : "presentation"}
        onclick={() => go_?.()}
        class="-mx-2 flex items-start gap-2.5 rounded-md px-2 py-2 text-left text-[12.5px] {go_
          ? 'hover:bg-(--bg-hover)'
          : ''}"
      >
        {@render avatar(a, 24)}
        <span class="min-w-0 flex-1 text-(--text-muted)"
          >{@render sentence(a)}</span
        >
        <small
          class="flex shrink-0 items-center gap-1.5 pt-0.5 text-[11px] text-(--text-faint)"
        >
          {#if !a.readAt}<i class="size-1.5 rounded-full bg-(--accent)"
            ></i>{/if}
          {timeAgo(a.createdAt)}
        </small>
      </svelte:element>
    {/each}
  </div>
{/if}
