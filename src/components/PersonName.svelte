<script lang="ts">
  import type { PublicUser } from "$lib/types";

  let {
    user,
    class: className = "font-medium",
  }: {
    user: Pick<PublicUser, "displayName" | "username">;
    /** For the display name: its weight and size. */
    class?: string;
  } = $props();
</script>

<!-- A row's name: the display name, then the @username nobody else can have. The @ gives
     way first when the row is narrow. -->
<span class="flex min-w-0 items-baseline gap-1.5">
  <b class="max-w-full min-w-0 shrink-0 truncate {className}">{user.displayName}</b>
  <span class="min-w-0 truncate text-[13px] text-(--text-faint)"
    >@{user.username}</span
  >
</span>
