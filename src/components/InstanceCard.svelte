<script lang="ts" module>
  import type { InstanceSummary } from "$lib/types";

  /** Cards that want the user's attention sort first on Home. */
  export function needsAttention(i: InstanceSummary) {
    const s = i.sync?.state;
    return (
      i.status !== "ready" ||
      s === "update_available" ||
      s === "conflict" ||
      (s === "local_changes" && i.sync?.role !== "viewer")
    );
  }
</script>

<script lang="ts">
  import { api } from "$lib/api";
  import { openDialog } from "$lib/dialogs.svelte";
  import {
    openLabel,
    plural,
    timeAgo,
    versionLabel,
    workLabel,
    workPercent,
  } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { kindFor, wallPixels } from "$lib/pixel";
  import { canPlay, play } from "$lib/play";
  import { instances, social } from "$lib/stores";
  import Button from "./Button.svelte";
  import Head from "./Head.svelte";
  import Icon from "./Icon.svelte";

  let { instance }: { instance: InstanceSummary } = $props();

  // Open in the launcher, right from the card. Folders have no launcher to open.
  const canOpen = $derived(
    instance.status !== "missing" && instance.launcher.kind !== "folder",
  );
  let opening = $state(false);
  let openError = $state<string | null>(null);
  async function openInLauncher() {
    opening = true;
    openError = null;
    try {
      await api.openInLauncher(instance.id);
    } catch (e) {
      openError = (e as Error).message;
    } finally {
      opening = false;
    }
  }

  const pixels = $derived(wallPixels(kindFor(instance.id), instance.id));
  // The launcher's picture for this instance, if it has one and it loads.
  let iconFailed = $state(false);
  $effect(() => {
    instance.icon;
    iconFailed = false;
  });
  // Only real image URLs; icon keys ("leaf") aren't pictures.
  const showIcon = $derived(
    !!instance.icon &&
      /^(https?:|data:image\/)/.test(instance.icon) &&
      !iconFailed,
  );
  const open = () => go({ page: "instance", id: instance.id });
  // Reading files after tracking, or uploading while sharing.
  const work = $derived(instances.work[instance.id]);

  // Who else has it, as faces on shared cards.
  const people = $derived.by(() => {
    const id = instance.sync?.sharedInstanceId;
    const shared = id ? social.shared.find((x) => x.id === id) : undefined;
    const me = social.session?.user?.id;
    return (shared?.members ?? []).filter(
      (m) => m.status === "active" && m.user.id !== me,
    );
  });

  type Status = {
    tone: "warn" | "accent" | "online" | "muted" | "shared" | "faint";
    text: string;
    action?: {
      label: string;
      variant: "warn" | "primary" | "default";
      run: () => void;
    };
  };

  // One plain-language line about this instance, plus a button only when there's
  // something to do. Order matters: the first match wins.
  const status = $derived.by((): Status => {
    const i = instance;
    const s = i.sync;
    if (i.status === "missing")
      return {
        tone: "warn",
        text: "Folder missing",
        action: { label: "Open", variant: "default", run: open },
      };
    if (i.isRunning) return { tone: "online", text: "Playing now" };
    if (s?.state === "update_available")
      return {
        tone: "warn",
        text:
          s.role === "owner"
            ? "An editor sent an update"
            : `${s.owner?.displayName ?? "The owner"} sent an update`,
        action: {
          label: "Update",
          variant: "warn",
          run: () => go({ page: "review", id: i.id }),
        },
      };
    if (s?.state === "conflict")
      return {
        tone: "warn",
        text: "Update available · would undo your changes",
        action: {
          label: "Review",
          variant: "warn",
          run: () => go({ page: "review", id: i.id }),
        },
      };
    if (s?.state === "local_changes" && s.role !== "viewer")
      return {
        tone: "accent",
        text: `${plural(s.localChangeCount, "change")} not sent yet`,
        action: {
          label: "Send",
          variant: "primary",
          run: () => openDialog({ kind: "publish", instanceId: i.id }),
        },
      };
    if (s?.state === "local_changes")
      return {
        tone: "muted",
        text: `${plural(s.localChangeCount, "local change")} on this PC only`,
      };
    if (s?.state === "frozen")
      return { tone: "muted", text: "No longer updated · the owner left" };
    if (s) {
      const shared = social.shared.find((x) => x.id === s.sharedInstanceId);
      const others = (shared?.members ?? []).filter(
        (m) => m.role !== "owner" && m.status === "active",
      );
      // Short enough for a narrow card: who, by name when it's one person.
      return {
        tone: "shared",
        text:
          s.role === "owner"
            ? others.length === 1
              ? `Shared with ${others[0].user.displayName}`
              : others.length
                ? `Shared with ${plural(others.length, "person", "people")}`
                : "Shared · nobody joined yet"
            : `From ${s.owner?.displayName ?? "a friend"}`,
      };
    }
    return {
      tone: "faint",
      text: instance.sharedElsewhere
        ? "Shared on another account"
        : "Not shared",
    };
  });

  const DOT = {
    warn: "bg-(--warn)",
    accent: "bg-(--accent)",
    online: "bg-(--online)",
    muted: "bg-(--text-faint)",
    shared: "bg-(--online)",
    // Hollow: nothing going on.
    faint: "border border-(--text-faint)",
  };
  // Coloured states are pills (dot and words, never colour alone); quiet ones plain text.
  const TEXT = {
    warn: "rounded-full bg-(--warn)/13 px-2 py-0.5 font-semibold text-(--warn)",
    accent:
      "rounded-full bg-(--accent)/14 px-2 py-0.5 font-semibold text-(--accent)",
    online:
      "rounded-full bg-(--online)/13 px-2 py-0.5 font-semibold text-(--online)",
    muted: "text-(--text-muted)",
    shared:
      "rounded-full bg-(--online)/13 px-2 py-0.5 font-semibold text-(--online)",
    faint: "text-(--text-faint)",
  };
</script>

<article
  class="group flex flex-col overflow-hidden rounded-xl border bg-(--bg-raise) shadow-[0_8px_24px_#0000004d] transition-colors hover:bg-(--bg-raise-2)
    {needsAttention(instance) ? 'border-(--warn)/45' : 'border-(--line)'}
    {instance.status === 'missing' ? 'opacity-75' : ''}"
>
  <!-- Cover and title open the instance; the buttons do their own thing. -->
  <div class="relative">
    <button
      onclick={open}
      class="relative block aspect-2/1 w-full overflow-hidden"
      tabindex="-1"
      aria-hidden="true"
    >
      {#if showIcon}
        <!-- The picture twice: blown up and blurred as the backdrop, crisp on top. -->
        <img
          src={instance.icon}
          alt=""
          aria-hidden="true"
          class="absolute inset-0 size-full scale-125 object-cover opacity-70 blur-2xl saturate-150 {instance.status ===
          'missing'
            ? 'grayscale'
            : ''}"
        />
        <img
          src={instance.icon}
          alt=""
          onerror={() => (iconFailed = true)}
          class="absolute top-1/2 left-1/2 size-18 -translate-x-1/2 -translate-y-1/2 rounded-2xl object-cover shadow-[0_8px_24px_#0008] transition-transform duration-300 group-hover:scale-[1.04] {instance.status ===
          'missing'
            ? 'grayscale'
            : ''}"
        />
      {:else}
        <svg
          viewBox="0 0 40 24"
          preserveAspectRatio="xMidYMid slice"
          shape-rendering="crispEdges"
          class="size-full transition-transform duration-300 group-hover:scale-[1.03] {instance.status ===
          'missing'
            ? 'grayscale'
            : ''}"
        >
          {#each pixels as p, i (i)}
            <rect x={p.x} y={p.y} width="1" height="1" fill={p.fill} />
          {/each}
        </svg>
      {/if}
      <div
        class="absolute inset-0 bg-linear-to-b from-transparent from-40% to-(--bg-raise)/80"
      ></div>
    </button>
    {#if canPlay(instance)}
      <button
        onclick={() => play(instance)}
        aria-label="Play {instance.name}"
        title="Play {instance.name}"
        class="absolute right-2.5 bottom-2.5 grid size-9 place-items-center rounded-full bg-(--accent) text-(--accent-ink) shadow-[0_6px_18px_#0009] transition-transform hover:scale-105 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--accent)"
      >
        <Icon name="play" class="ml-0.5 size-4.5" />
      </button>
    {/if}
  </div>

  <div class="flex flex-1 flex-col px-3.5 pt-2.5 pb-3">
    <div class="mb-3">
      <button onclick={open} class="block w-full text-left">
        <b
          class="block truncate text-[14px] font-semibold"
          title={instance.name}>{instance.name}</b
        >
      </button>
      <!-- The launcher's name opens the instance there. -->
      <small
        class="flex min-w-0 items-center gap-1 text-[13px] text-(--text-muted)"
      >
        <span class="shrink-0"
          >{versionLabel(instance.loader, instance.minecraftVersion)} ·</span
        >
        {#if canOpen}
          <button
            onclick={openInLauncher}
            disabled={opening}
            title={openLabel(instance.launcher)}
            class="inline-flex min-w-0 items-center gap-1 rounded transition-colors hover:text-(--accent) disabled:opacity-60"
          >
            <span class="truncate">{instance.launcher.name}</span>
            <Icon name="external" class="size-3 shrink-0" />
          </button>
        {:else}
          <span class="truncate">{instance.launcher.name}</span>
        {/if}
      </small>
      <button
        onclick={open}
        tabindex="-1"
        class="mt-1.5 flex w-full flex-wrap items-center gap-x-3 gap-y-0.5 text-left text-[13px] whitespace-nowrap text-(--text-faint)"
      >
        <span class="flex items-center gap-1">
          <Icon name="cube" class="size-3" />
          {plural(instance.contentCount, "file")}
        </span>
        {#if instance.lastPlayedAt}
          <span class="flex items-center gap-1">
            <Icon name="clock" class="size-3" />
            Played {timeAgo(instance.lastPlayedAt)}
          </span>
        {/if}
      </button>
    </div>

    <div class="mt-auto border-t border-(--line) pt-2.5">
      {#if instance.progress?.stage === "waiting_for_manual_downloads"}
        <p class="flex min-h-7 items-center text-[13px] text-(--warn)">
          {instance.progress.manualDownloads.length === 1
            ? "Download 1 file to finish: open it to see how"
            : `Download ${instance.progress.manualDownloads.length} files to finish: open it to see how`}
        </p>
      {:else if instance.progress?.stage === "waiting_for_game_exit"}
        <p class="flex min-h-7 items-center text-[13px] text-(--warn)">
          Close Minecraft to finish updating
        </p>
      {:else if instance.status === "syncing" && instance.progress}
        <div
          class="flex items-center justify-between text-[13px] text-(--text-muted)"
        >
          <span>Updating…</span><span class="tabular-nums"
            >{instance.progress.percent}%</span
          >
        </div>
        <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-(--line)">
          <div
            class="h-full bg-(--accent) transition-[width] duration-300"
            style="width: {instance.progress.percent}%"
          ></div>
        </div>
      {:else if work}
        {@const percent = workPercent(work)}
        <div class="flex min-h-7 flex-col justify-center">
          <span class="truncate text-[13px] text-(--text-muted) tabular-nums"
            >{workLabel(work)}</span
          >
          <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-(--line)">
            <div
              class="h-full bg-(--accent) transition-[width] duration-300 {percent ===
              null
                ? 'w-full animate-pulse'
                : ''}"
              style={percent === null ? undefined : `width: ${percent}%`}
            ></div>
          </div>
        </div>
      {:else}
        <div class="flex min-h-7 flex-wrap items-center gap-x-2.5 gap-y-2">
          {#if openError}
            <span
              class="line-clamp-2 min-w-0 flex-1 text-[13px] text-(--danger)"
              role="alert"
              title={openError}>{openError}</span
            >
          {:else}
            <span class="flex max-w-full min-w-0 shrink-0">
              <span
                class="flex min-w-0 items-center gap-1.5 text-[13px] {TEXT[
                  status.tone
                ]}"
              >
                <i class="size-2 shrink-0 rounded-full {DOT[status.tone]}"></i>
                <span class="line-clamp-2">{status.text}</span>
              </span>
            </span>
          {/if}
          {#if !status.action && people.length}
            <span
              class="flex shrink-0 items-center gap-1"
              title={people.map((m) => m.user.displayName).join(", ")}
            >
              <!-- Three slots at most: faces, then a count. Side by side, since
                   square heads and round avatars don't overlap neatly. -->
              {#each people.slice(0, people.length > 3 ? 2 : 3) as m (m.user.id)}
                <Head user={m.user} size={20} />
              {/each}
              {#if people.length > 3}
                <span
                  class="grid h-5 place-items-center rounded-full bg-(--line) px-1.5 text-[13px] font-semibold text-(--text-primary) tabular-nums"
                  >+{people.length - 2}</span
                >
              {/if}
            </span>
          {/if}
          {#if status.action}
            <Button
              class="ml-auto"
              size="sm"
              variant={status.action.variant}
              onclick={status.action.run}>{status.action.label}</Button
            >
          {/if}
        </div>
      {/if}
    </div>
  </div>
</article>
