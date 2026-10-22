<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { formatCode, normalizeCode } from "$lib/format";
  import { social } from "$lib/stores";
  import type { PublicUser } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";

  const me = $derived(social.session?.user);
  // Depend on the code itself, not the whole session object: a $derived only
  // notifies when its value changes, so reloading the session doesn't re-run the lookup.
  const myCode = $derived(me?.friendCode);
  const clip = copier();

  let input = $state("");
  const code = $derived(normalizeCode(input));

  type Lookup =
    | { status: "idle" | "loading" | "missing" | "self" }
    | { status: "found"; user: PublicUser };
  let lookup = $state<Lookup>({ status: "idle" });

  // Look the code up as soon as it's complete, but only after typing pauses.
  $effect(() => {
    const c = code; // dependency: re-run whenever the code changes
    if (c.length !== 8) {
      lookup = { status: "idle" };
      return;
    }
    if (c === myCode) {
      lookup = { status: "self" };
      return;
    }
    lookup = { status: "loading" };
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const user = await api.previewFriendCode(c);
        if (!cancelled) lookup = { status: "found", user };
      } catch {
        if (!cancelled) lookup = { status: "missing" };
      }
    }, 300);
    // Cleanup: runs before the next run (the code changed) and when the card goes away.
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  // What the found person already is to you decides the button.
  const relation = $derived.by(() => {
    if (lookup.status !== "found") return null;
    const id = lookup.user.id;
    if (social.friends.some((f) => f.user.id === id)) return "friend";
    if (social.incomingRequests.some((r) => r.user.id === id))
      return "incoming";
    if (social.outgoingRequests.some((r) => r.user.id === id)) return "pending";
    return "new";
  });

  let sending = $state(false);
  let error = $state<string | null>(null);
  let sentTo = $state<string | null>(null);

  async function send() {
    // Read what we need *before* awaiting: state can change while the request runs.
    const name = lookup.status === "found" ? lookup.user.displayName : null;
    sending = true;
    error = null;
    try {
      await social.sendFriendRequest(code);
      sentTo = name;
      input = "";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      sending = false;
    }
  }
</script>

<div class="rounded-xl bg-(--bg-raise) p-4.5">
  <h2 class="mb-1 font-semibold">Add a friend</h2>
  <p class="mb-3.5 text-[12.5px] text-(--text-muted)">
    Swap friend codes. A code only sends a request; nobody is added until they
    accept.
  </p>

  {#if me}
    <div
      class="flex items-center justify-between gap-2.5 rounded-lg bg-(--bg) px-3.5 py-2.5"
    >
      <span class="font-mono text-xl font-semibold tracking-[2px]"
        >{formatCode(me.friendCode)}</span
      >
      <div class="flex gap-0.5">
        <button
          title="Copy code"
          aria-label="Copy code"
          onclick={() => clip.copy(formatCode(me.friendCode), "code")}
          class="grid size-8 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
        >
          <Icon
            name={clip.copied === "code" ? "check" : "folders"}
            class="size-4"
          />
        </button>
        <button
          title="Copy invite link"
          aria-label="Copy invite link"
          onclick={() => clip.copy(me.friendLink, "link")}
          class="grid size-8 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
        >
          <Icon
            name={clip.copied === "link" ? "check" : "external"}
            class="size-4"
          />
        </button>
      </div>
    </div>
    <p
      class="mt-1.5 min-h-4 text-[11.5px] {clip.failed
        ? 'text-(--warn)'
        : 'text-(--online)'}"
    >
      {#if clip.copied === "code"}Code copied
      {:else if clip.copied === "link"}Invite link copied
      {:else if clip.failed === "code"}Couldn't copy. Select the code above and
        copy it yourself.
      {:else if clip.failed === "link"}Couldn't copy. Your link: {me.friendLink}
      {/if}
    </p>
  {/if}

  <input
    bind:value={input}
    oninput={() => (sentTo = null)}
    placeholder="Their code or link"
    aria-label="Friend code"
    autocomplete="off"
    class="mt-1 h-8.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono tracking-wider uppercase outline-none
      placeholder:font-(family-name:--font) placeholder:tracking-normal placeholder:normal-case focus:border-(--accent)"
  />

  {#if lookup.status === "loading"}
    <p class="mt-2 text-xs text-(--text-faint)">
      Looking up {formatCode(code)}…
    </p>
  {:else if lookup.status === "missing"}
    <p class="mt-2 text-xs text-(--danger)">
      No one has that code. Check it with your friend; codes can be reset.
    </p>
  {:else if lookup.status === "self"}
    <p class="mt-2 text-xs text-(--text-muted)">That's your own code.</p>
  {:else if lookup.status === "found"}
    <div class="mt-3 flex items-center gap-3 rounded-lg bg-(--bg) px-3 py-2.5">
      <Head user={lookup.user} size={32} />
      <div class="min-w-0 flex-1">
        <b class="block font-medium">{lookup.user.displayName}</b>
        <small class="text-[11.5px] text-(--text-faint)"
          >@{lookup.user.username}</small
        >
      </div>
      {#if relation === "friend"}
        <span class="text-xs text-(--online)">Already friends</span>
      {:else if relation === "pending"}
        <span class="text-xs text-(--text-faint)">Request sent</span>
      {:else}
        <Button size="sm" variant="primary" disabled={sending} onclick={send}>
          {relation === "incoming" ? "Accept their request" : "Send request"}
        </Button>
      {/if}
    </div>
  {:else if sentTo}
    <p class="mt-2 text-xs text-(--online)">Request sent to {sentTo}.</p>
  {/if}
  {#if error}<p class="mt-2 text-xs text-(--danger)" role="alert">
      {error}
    </p>{/if}
</div>
