<script lang="ts">
  import { social } from "$lib/stores";
  import type { PublicUser } from "$lib/types";
  import Head from "./Head.svelte";

  /** A person named inside a sentence. Resting the pointer on them shows who it is: their
   *  face, names, and whether they're your friend. */
  let { user }: { user: PublicUser } = $props();
  // Its weight and colour come from the sentence around it.

  let at = $state<{ left: number; bottom: number } | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Fixed to the window, so a card or list that clips its overflow doesn't cut it off.
  function show(e: Event) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    clearTimeout(timer);
    timer = setTimeout(() => {
      at = {
        left: Math.max(8, Math.min(rect.left, window.innerWidth - 268)),
        bottom: window.innerHeight - rect.top + 6,
      };
    }, 250);
  }
  function hide() {
    clearTimeout(timer);
    at = null;
  }

  const relation = $derived(
    user.id === social.session?.user?.id
      ? "You"
      : social.friends.some((f) => f.user.id === user.id)
        ? "Friend"
        : "Not your friend",
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (the card only repeats the name and @username, which screen readers get from the text) -->
<span
  class="decoration-(--text-faint) underline-offset-3 hover:underline"
  onmouseenter={show}
  onmouseleave={hide}
  >{user.displayName}<span class="sr-only"> (@{user.username})</span></span
>
{#if at}
  <span
    class="pointer-events-none fixed z-50 flex w-max max-w-64 items-center gap-2.5 rounded-lg bg-(--bg-raise-2) py-2.5 pr-3.5 pl-2.5 text-left font-normal shadow-(--shadow-menu)"
    style="left:{at.left}px;bottom:{at.bottom}px"
    aria-hidden="true"
  >
    <Head {user} size={32} ring="var(--bg-raise-2)" />
    <span class="min-w-0">
      <b class="block truncate font-semibold text-(--text-primary)"
        >{user.displayName}</b
      >
      <small class="block truncate text-[13px] text-(--text-faint)"
        >@{user.username} · {relation}</small
      >
    </span>
  </span>
{/if}
