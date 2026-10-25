<script lang="ts">
  import type { PresenceState, PublicUser } from "$lib/types";

  let {
    user,
    size = 28,
    presence = null,
    ring = "var(--bg)",
  }: {
    user: Pick<PublicUser, "displayName" | "skinUrl" | "avatarUrl">;
    size?: number;
    presence?: PresenceState | null;
    /** Background behind the presence dot, so it reads as cut out. */
    ring?: string;
  } = $props();

  // A skin is 64 px wide with the face at (8, 8) and the hat layer over it at (40, 8),
  // each 8 px square: scale it up 8 × size / 64 and shift each one into view.
  const layer = (x: number) =>
    `background-image:url("${user.skinUrl}");background-size:${size * 8}px auto;` +
    `background-position:-${(x / 8) * size}px -${size}px`;
</script>

<!-- The face of their Minecraft skin, their Discord avatar, or initials. -->
<span
  class="relative inline-block shrink-0"
  style="width:{size}px;height:{size}px"
>
  {#if user.skinUrl}
    <span
      class="relative block size-full overflow-hidden rounded-[4px] bg-(--bg-raise-2)"
      aria-hidden="true"
    >
      <span class="skin absolute inset-0" style={layer(8)}></span>
      <span class="skin absolute inset-0" style={layer(40)}></span>
    </span>
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

<style>
  .skin {
    background-repeat: no-repeat;
    image-rendering: pixelated;
  }
</style>
