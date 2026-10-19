<script lang="ts">
  import { kindFor, wallPixels } from "$lib/pixel";
  import { go } from "$lib/nav.svelte";
  import { timeAgo, versionLabel } from "$lib/format";
  import type { InstanceSummary } from "$lib/types";

  let { instance }: { instance: InstanceSummary } = $props();

  const pixels = $derived(wallPixels(kindFor(instance.id), instance.id));

  // One badge at most, in priority order. $derived.by takes a function body
  // for when the value needs more than one expression to work out.
  const badge = $derived.by(() => {
    const s = instance.sync;
    if (instance.status === "syncing")
      return { text: "Updating…", tone: "text-(--accent)" };
    if (s?.state === "update_available" || s?.state === "conflict")
      return { text: "Update ready", tone: "text-(--warn)" };
    if (s?.state === "local_changes")
      return { text: "Unpublished changes", tone: "text-(--accent)" };
    if (s?.state === "frozen")
      return { text: "Frozen", tone: "text-(--text-muted)" };
    return null;
  });
</script>

<button
  onclick={() => go({ page: "instance", id: instance.id })}
  class="group rounded-xl text-left"
>
  <div class="relative aspect-[5/3] overflow-hidden rounded-xl bg-black">
    <svg
      viewBox="0 0 40 24"
      preserveAspectRatio="xMidYMid slice"
      shape-rendering="crispEdges"
      class="size-full transition-transform duration-300 group-hover:scale-[1.04]"
      aria-hidden="true"
    >
      {#each pixels as p, i (i)}
        <rect x={p.x} y={p.y} width="1" height="1" fill={p.fill} />
      {/each}
    </svg>
    <!-- darken the bottom so the badge reads on light blocks -->
    <div
      class="absolute inset-0 bg-linear-to-b from-transparent from-35% to-black/55"
    ></div>
    {#if badge}
      <span
        class="absolute bottom-2.5 left-2.5 inline-flex h-5.5 items-center gap-1.5 rounded-full bg-[#0b0d10e6] px-2 text-[11.5px] font-semibold {badge.tone}"
      >
        <i class="size-1.5 rounded-full bg-current"></i>{badge.text}
      </span>
    {/if}
  </div>
  <div class="px-0.5 pt-2.5">
    <b class="block truncate text-[13.5px] font-semibold">{instance.name}</b>
    <small class="block truncate text-xs text-(--text-muted)">
      {versionLabel(instance.loader, instance.minecraftVersion)} · {instance
        .launcher.name} · {timeAgo(instance.lastPlayedAt)}
    </small>
  </div>
</button>
