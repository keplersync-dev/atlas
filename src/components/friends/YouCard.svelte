<script lang="ts">
  // The top of the Friends page: you (and how friends see you), your friend code, and a
  // box for theirs. Everything about adding people is here.
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

  // How friends see you: online, or appearing offline.
  let statusOpen = $state(false);
  let statusRoot = $state<HTMLDivElement>();
  let statusError = $state<string | null>(null);
  async function setAppearOffline(next: boolean) {
    statusOpen = false;
    statusError = null;
    try {
      await social.updateProfile({ appearOffline: next });
    } catch (e) {
      statusError = (e as Error).message;
    }
  }

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

<svelte:window
  onclick={(e) =>
    statusOpen &&
    !statusRoot?.contains(e.target as Node) &&
    (statusOpen = false)}
  onkeydown={(e) => e.key === "Escape" && (statusOpen = false)}
/>

{#if me}
  <section class="rounded-xl bg-(--bg-raise) px-4 py-3.5">
    <div class="flex flex-wrap items-center gap-x-4 gap-y-3">
      <!-- You, and how friends see you -->
      <div class="flex items-center gap-3">
        <Head user={me} size={40} ring="var(--bg-raise)" />
        <div>
          <b class="block font-semibold">{me.displayName}</b>
          <div class="relative" bind:this={statusRoot}>
            <button
              aria-haspopup="menu"
              aria-expanded={statusOpen}
              onclick={() => (statusOpen = !statusOpen)}
              class="-ml-1 flex items-center gap-1.5 rounded px-1 text-xs text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
            >
              <span
                class="size-2 rounded-full {me.appearOffline
                  ? 'bg-(--text-faint)'
                  : 'bg-(--online)'}"
              ></span>
              {me.appearOffline ? "Appearing offline" : "Online"}
              <Icon name="chev" class="size-3 rotate-90" />
            </button>
            {#if statusOpen}
              <div
                role="menu"
                class="absolute top-full left-0 z-30 mt-1.5 w-64 rounded-lg border border-(--line-2) bg-[#1b1f25] p-1.25 shadow-[0_16px_40px_#0009]"
              >
                {#each [{ offline: false, label: "Online", hint: "Friends see when you're on and what you're playing" }, { offline: true, label: "Appear offline", hint: "Friends see you as offline" }] as o (o.label)}
                  <button
                    role="menuitemradio"
                    aria-checked={me.appearOffline === o.offline}
                    onclick={() => setAppearOffline(o.offline)}
                    class="flex w-full items-start gap-2.5 rounded-[5px] px-2 py-1.5 text-left hover:bg-(--bg-hover)"
                  >
                    <span
                      class="mt-1.5 size-2 shrink-0 rounded-full {o.offline
                        ? 'bg-(--text-faint)'
                        : 'bg-(--online)'}"
                    ></span>
                    <span>
                      <b class="block text-[12.5px] font-medium">{o.label}</b>
                      <small class="text-[11.5px] text-(--text-faint)"
                        >{o.hint}</small
                      >
                    </span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>

      <span class="h-9 w-px bg-(--line-2)"></span>

      <!-- Your code -->
      <div>
        <small class="block text-[11px] text-(--text-faint)"
          >Your friend code</small
        >
        <span class="font-mono text-lg font-semibold tracking-[2px]"
          >{formatCode(me.friendCode)}</span
        >
      </div>
      <div class="flex gap-1.5">
        <Button
          size="sm"
          class="gap-1.5"
          onclick={() => clip.copy(formatCode(me.friendCode), "code")}
        >
          {#if clip.copied === "code"}<Icon
              name="check"
              class="size-3.5"
            />{/if}
          Copy code
        </Button>
        <Button
          size="sm"
          class="gap-1.5"
          onclick={() => clip.copy(me.friendLink, "link")}
        >
          {#if clip.copied === "link"}<Icon
              name="check"
              class="size-3.5"
            />{/if}
          Copy link
        </Button>
      </div>

      <!-- Their code -->
      <input
        bind:value={input}
        oninput={() => (sentTo = null)}
        placeholder="Add a friend: paste their code"
        aria-label="Friend code"
        autocomplete="off"
        class="ml-auto h-8.5 w-64 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono tracking-wider uppercase outline-none
          placeholder:font-(family-name:--font) placeholder:tracking-normal placeholder:normal-case focus:border-(--accent)"
      />
    </div>

    <!-- Results and messages, under the row -->
    {#if clip.failed === "code"}
      <p class="mt-2.5 text-xs text-(--warn)">
        Couldn't copy. Select the code and copy it yourself.
      </p>
    {:else if clip.failed === "link"}
      <p class="mt-2.5 text-xs text-(--warn)">
        Couldn't copy. Your link: {me.friendLink}
      </p>
    {/if}
    {#if statusError}<p class="mt-2.5 text-xs text-(--danger)" role="alert">
        {statusError}
      </p>{/if}
    {#if lookup.status === "loading"}
      <p class="mt-2.5 text-xs text-(--text-faint)">
        Looking up {formatCode(code)}…
      </p>
    {:else if lookup.status === "missing"}
      <p class="mt-2.5 text-xs text-(--danger)">
        No one has that code. Check it with your friend; codes can be reset.
      </p>
    {:else if lookup.status === "self"}
      <p class="mt-2.5 text-xs text-(--text-muted)">That's your own code.</p>
    {:else if lookup.status === "found"}
      <div
        class="mt-3 flex items-center gap-3 rounded-lg bg-(--bg) px-3 py-2.5"
      >
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
      <p class="mt-2.5 text-xs text-(--online)">Request sent to {sentTo}.</p>
    {/if}
    {#if error}<p class="mt-2.5 text-xs text-(--danger)" role="alert">
        {error}
      </p>{/if}
  </section>
{/if}
