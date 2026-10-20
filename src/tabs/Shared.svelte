<script lang="ts" module>
  // Which groups the user opened or closed. Module-level, so it survives leaving
  // the page. A group with no entry here opens by itself when it needs attention.
  let expanded = $state<Record<string, boolean>>({});
</script>

<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { fileSize, plural, timeAgo, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { instances, social } from "$lib/stores";
  import type { InstanceSummary, PublicUser, SharedInstance } from "$lib/types";
  import BlockArt from "../components/BlockArt.svelte";
  import Button from "../components/Button.svelte";
  import Head from "../components/Head.svelte";
  import Icon from "../components/Icon.svelte";

  type Tone = "accent" | "warn" | "online" | "muted";
  type RowState = {
    tone: Tone;
    text: string;
    action?: { label: string; variant: "primary" | "warn"; run: () => void };
    /** Counts toward the group's "needs attention" badge. */
    attention?: "new" | "update" | "changes";
  };
  type Group = {
    key: string;
    title: string;
    user: PublicUser | null;
    items: SharedInstance[];
    mine: boolean;
  };

  const localFor = (s: SharedInstance): InstanceSummary | undefined =>
    s.localInstanceId ? instances.byId(s.localInstanceId) : undefined;

  /** One plain-language line per instance, plus a button only when there's something to do. */
  function rowState(s: SharedInstance): RowState {
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

  // "Shared by you" first, then one group per friend who shared something with you.
  const groups = $derived.by((): Group[] => {
    const me = social.session?.user ?? null;
    const mine = social.shared.filter((s) => s.myRole === "owner");
    const byOwner = new Map<string, Group>();
    for (const s of social.shared) {
      if (s.myRole === "owner" || s.myStatus === "declined") continue;
      const key = s.owner?.id ?? "former";
      if (!byOwner.has(key))
        byOwner.set(key, {
          key,
          title: s.owner?.displayName ?? "Former members",
          user: s.owner,
          items: [],
          mine: false,
        });
      byOwner.get(key)!.items.push(s);
    }
    const friendGroups = [...byOwner.values()].sort((a, b) => {
      // Groups that need you first, then alphabetical; "Former members" last.
      if (a.key === "former") return 1;
      if (b.key === "former") return -1;
      const need = attentionOf(b).total - attentionOf(a).total;
      return need || a.title.localeCompare(b.title);
    });
    return [
      {
        key: "mine",
        title: "Shared by you",
        user: me,
        items: mine,
        mine: true,
      },
      ...friendGroups,
    ];
  });

  function attentionOf(g: Group) {
    const counts = { new: 0, update: 0, changes: 0, total: 0 };
    for (const s of g.items) {
      const a = rowState(s).attention;
      if (a) {
        counts[a]++;
        counts.total++;
      }
    }
    return counts;
  }

  const isOpen = (g: Group) => expanded[g.key] ?? attentionOf(g).total > 0;
  const toggle = (g: Group) => (expanded[g.key] = !isOpen(g));

  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  async function decline(s: SharedInstance) {
    busy = s.id;
    error = null;
    try {
      await social.declineInvite(s.id);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = null;
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

<div class="mb-6">
  <h1
    class="font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
  >
    Shared
  </h1>
  <p class="mt-1.5 text-(--text-muted)">
    What you share, and what your friends share with you.
  </p>
</div>

{#if !social.signedIn}
  <div class="py-12 text-center text-(--text-muted)">
    <b class="block text-sm text-(--text-primary)">Sign in to share instances</b
    >
    Your tracked instances keep working without an account.
    <div>
      <Button
        variant="primary"
        class="mt-4"
        onclick={() => social.signIn("minecraft")}>Sign in</Button
      >
    </div>
  </div>
{:else}
  {#if error}<p class="mb-3 text-xs text-(--danger)" role="alert">
      {error}
    </p>{/if}

  <div class="flex flex-col gap-3">
    {#each groups as g (g.key)}
      {@const open = isOpen(g)}
      {@const need = attentionOf(g)}
      <section class="overflow-hidden rounded-xl bg-(--bg-raise)">
        <!-- Group header: the whole bar toggles. aria-expanded tells screen readers. -->
        <button
          onclick={() => toggle(g)}
          aria-expanded={open}
          class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-(--bg-raise-2)"
        >
          <span
            class="text-(--text-faint) transition-transform {open
              ? 'rotate-90'
              : ''}"
          >
            <Icon name="chev" class="size-3.5" />
          </span>
          {#if g.user}
            <Head user={g.user} size={28} ring="var(--bg-raise)" />
          {:else}
            <span
              class="grid size-7 place-items-center rounded-full bg-(--bg-raise-2) text-(--text-faint)"
              >?</span
            >
          {/if}
          <span class="min-w-0 flex-1">
            <b class="font-semibold">{g.title}</b>
            <span class="ml-1.5 text-(--text-faint)">{g.items.length}</span>
          </span>
          {#if need.new}<span
              class="rounded-full bg-(--accent)/15 px-2 py-0.5 text-[11px] font-semibold text-(--accent)"
              >{need.new} new</span
            >{/if}
          {#if need.update}<span
              class="rounded-full bg-(--warn)/15 px-2 py-0.5 text-[11px] font-semibold text-(--warn)"
              >{plural(need.update, "update")}</span
            >{/if}
          {#if need.changes}<span
              class="rounded-full bg-(--accent)/15 px-2 py-0.5 text-[11px] font-semibold text-(--accent)"
              >{plural(need.changes, "to publish", "to publish")}</span
            >{/if}
        </button>

        {#if open}
          <div class="border-t border-(--line)">
            {#each g.items as s (s.id)}
              {@const st = rowState(s)}
              {@const local = localFor(s)}
              <div
                class="flex items-center gap-3.5 px-4 py-2.5 not-last:border-b not-last:border-(--line)"
              >
                <!-- Click: open it if it's on this PC, or start adding it if it's a new invite.
                     Otherwise there's nothing to open, so it's plain text (not a disabled button,
                     which would look faded). -->
                <svelte:element
                  this={local || s.myStatus === "invited" ? "button" : "div"}
                  role={local || s.myStatus === "invited"
                    ? undefined
                    : "presentation"}
                  onclick={() =>
                    local
                      ? go({ page: "instance", id: local.id })
                      : s.myStatus === "invited" &&
                        openDialog({ kind: "accept", sharedInstanceId: s.id })}
                  class="flex min-w-0 flex-1 items-center gap-3.5 rounded-md text-left"
                >
                  <BlockArt
                    seed={s.localInstanceId ?? s.id}
                    class="size-10 rounded-lg"
                  />
                  <span class="min-w-0 flex-1">
                    <b class="block truncate font-semibold">{s.name}</b>
                    <small class="block truncate text-xs text-(--text-faint)">
                      {versionLabel(s.loader, s.minecraftVersion)} · {plural(
                        s.contentCount,
                        "item",
                      )}
                      {#if s.optionalItems.length}· {s.optionalItems.length} optional{/if}
                      {#if s.myStatus === "invited"}· {fileSize(
                          s.sizeBytes,
                        )}{/if}
                      {#if s.description}· {s.description}{/if}
                    </small>
                  </span>
                </svelte:element>
                <span
                  class="flex shrink-0 items-center gap-1.5 text-xs {TONE[
                    st.tone
                  ]}"
                >
                  <i class="size-1.5 rounded-full {DOT[st.tone]}"></i>{st.text}
                </span>
                <span class="flex w-30 shrink-0 items-center justify-end gap-1">
                  {#if st.action}
                    <Button
                      size="sm"
                      variant={st.action.variant}
                      onclick={st.action.run}>{st.action.label}</Button
                    >
                  {/if}
                  {#if s.myStatus === "invited"}
                    <button
                      onclick={() => decline(s)}
                      disabled={busy === s.id}
                      title="Decline"
                      aria-label="Decline {s.name}"
                      class="grid size-7 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--text-primary) disabled:opacity-40"
                    >
                      <Icon name="close" class="size-3.5" />
                    </button>
                  {/if}
                </span>
              </div>
            {:else}
              <p class="px-4 py-4 text-[12.5px] text-(--text-muted)">
                {g.mine
                  ? "You haven't shared anything yet. Open an instance and choose Share."
                  : "Nothing here."}
              </p>
            {/each}
          </div>
        {/if}
      </section>
    {/each}

    {#if groups.length === 1}
      <p class="py-6 text-center text-[12.5px] text-(--text-muted)">
        When a friend shares an instance with you or a group you're in, it shows
        up here under their name.
      </p>
    {/if}
  </div>
{/if}
