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
  import { plural, timeAgo, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { kindFor, wallPixels } from "$lib/pixel";
  import { social } from "$lib/stores";
  import Button from "./Button.svelte";
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

  type Status = {
    tone: "warn" | "accent" | "online" | "muted";
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
        action: { label: "Fix", variant: "default", run: open },
      };
    if (i.isRunning) return { tone: "online", text: "Playing now" };
    if (s?.state === "update_available")
      return {
        tone: "warn",
        text: `${s.owner?.displayName ?? "The owner"} published an update`,
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
        text: `${plural(s.localChangeCount, "change")} not shared yet`,
        action: {
          label: "Publish",
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
      return { tone: "muted", text: "Frozen · the owner left" };
    if (s) {
      const shared = social.shared.find((x) => x.id === s.sharedInstanceId);
      const others = shared
        ? shared.members.filter(
            (m) => m.role !== "owner" && m.status === "active",
          ).length
        : 0;
      return {
        tone: "muted",
        text:
          s.role === "owner"
            ? `Shared with ${plural(others, "person", "people")} · up to date`
            : `From ${s.owner?.displayName ?? "a friend"} · up to date`,
      };
    }
    return { tone: "muted", text: "Not shared" };
  });

  const DOT = {
    warn: "bg-(--warn)",
    accent: "bg-(--accent)",
    online: "bg-(--online)",
    muted: "bg-(--text-faint)",
  };
  const TEXT = {
    warn: "text-(--warn)",
    accent: "text-(--accent)",
    online: "text-(--online)",
    muted: "text-(--text-muted)",
  };
</script>

<article
  class="group flex flex-col overflow-hidden rounded-xl bg-(--bg-raise) transition-colors hover:bg-(--bg-raise-2)
    {instance.status === 'missing' ? 'opacity-75' : ''}"
>
  <!-- Cover and title open the instance; the buttons do their own thing. -->
  <div class="relative">
    <button
      onclick={open}
      class="relative block aspect-16/7 w-full overflow-hidden"
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
          class="absolute top-1/2 left-1/2 size-24 -translate-x-1/2 -translate-y-1/2 rounded-2xl object-cover shadow-[0_8px_24px_#0008] transition-transform duration-300 group-hover:scale-[1.04] {instance.status ===
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
    {#if canOpen}
      <button
        onclick={openInLauncher}
        disabled={opening}
        class="absolute top-2.5 right-2.5 flex h-7 items-center gap-1.5 rounded-md bg-black/55 px-2.5 text-xs font-medium text-white backdrop-blur-sm transition-colors hover:bg-black/75 disabled:opacity-60"
      >
        <Icon name="external" class="size-3.5" />Open in {instance.launcher
          .name}
      </button>
    {/if}
  </div>

  <div class="flex flex-1 flex-col px-3.5 pt-2.5 pb-3">
    <button onclick={open} class="text-left">
      <b class="block truncate text-[14px] font-semibold">{instance.name}</b>
      <small class="block truncate text-xs text-(--text-faint)">
        {versionLabel(instance.loader, instance.minecraftVersion)} · {instance
          .launcher.name}
        {#if instance.lastPlayedAt}· {timeAgo(instance.lastPlayedAt)}{/if}
      </small>
    </button>

    <div class="mt-auto pt-3">
      {#if instance.status === "syncing" && instance.progress}
        <div
          class="flex items-center justify-between text-xs text-(--text-muted)"
        >
          <span>Updating…</span><span class="tabular-nums"
            >{instance.progress.percent}%</span
          >
        </div>
        <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-(--line-2)">
          <div
            class="h-full bg-(--accent) transition-[width] duration-300"
            style="width: {instance.progress.percent}%"
          ></div>
        </div>
      {:else}
        <div class="flex min-h-7 items-center gap-2.5">
          <span
            class="flex min-w-0 flex-1 items-center gap-1.5 text-xs {TEXT[
              status.tone
            ]}"
          >
            {#if openError}
              <span class="truncate text-(--danger)" title={openError}
                >{openError}</span
              >
            {:else}
              <i class="size-1.5 shrink-0 rounded-full {DOT[status.tone]}"></i>
              <span class="truncate">{status.text}</span>
            {/if}
          </span>
          {#if status.action}
            <Button
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
