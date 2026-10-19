<script lang="ts">
  import { headPixels } from "$lib/pixel";
  import type { PresenceState, PublicUser } from "$lib/types";

  let {
    user,
    size = 28,
    presence = null,
    ring = "var(--bg)",
  }: {
    user: Pick<PublicUser, "displayName" | "minecraftUsername" | "avatarUrl">;
    size?: number;
    presence?: PresenceState | null;
    /** Background behind the presence dot, so it reads as cut out. */
    ring?: string;
  } = $props();

  const pixels = $derived(
    user.minecraftUsername ? headPixels(user.minecraftUsername) : null,
  );
</script>

<!-- Minecraft head if linked; Discord avatar or initials otherwise. -->
<span
  class="relative inline-block shrink-0"
  style="width:{size}px;height:{size}px"
>
  {#if pixels}
    <svg
      viewBox="0 0 8 8"
      class="block size-full rounded-[4px]"
      shape-rendering="crispEdges"
      aria-hidden="true"
    >
      {#each pixels as p (p.x * 8 + p.y)}
        <rect x={p.x} y={p.y} width="1" height="1" fill={p.fill} />
      {/each}
    </svg>
  {:else if user.avatarUrl}
    <img
      src={user.avatarUrl}
      alt=""
      class="size-full rounded-full object-cover"
    />
  {:else}
    <span
      class="grid size-full place-items-center rounded-full bg-[#5865f2] font-semibold text-white"
      style="font-size:{Math.round(size * 0.42)}px">{user.displayName[0]}</span
    >
  {/if}
  {#if presence && presence !== "offline"}
    <i
      class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full bg-(--online)"
      style="box-shadow: 0 0 0 2px {ring}"
    ></i>
  {/if}
</span>
