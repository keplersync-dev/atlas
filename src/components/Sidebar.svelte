<script lang="ts">
  import { api } from "$lib/api";
  import { go, isActive, type View } from "$lib/nav.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { instances, social } from "$lib/stores";
  import type { InstanceSummary } from "$lib/types";
  import ContentIcon from "./ContentIcon.svelte";
  import Head from "./Head.svelte";
  import Icon from "./Icon.svelte";

  let signingIn = $state(false);
  let signInError = $state<string | null>(null);
  let signInAttempt = 0;

  const links: { view: View; label: string; icon: string }[] = [
    { view: { page: "home" }, label: "Home", icon: "home" },
    { view: { page: "shared" }, label: "Shared", icon: "inbox" },
    { view: { page: "friends" }, label: "Friends", icon: "users" },
  ];

  /** Count shown next to a nav link: invites on Shared, friend requests on Friends. */
  function badge(view: View) {
    if (view.page === "shared") return social.invites.length;
    if (view.page === "friends") return social.incomingRequests.length;
    return 0;
  }

  /** Something waiting on the user: an update to review or changes to publish. */
  const needsYou = (i: InstanceSummary) =>
    instances.needingAttention.includes(i);

  function openLegal(page: "terms" | "privacy") {
    api.openLegalPage(page).catch((e) => (signInError = (e as Error).message));
  }

  /** Opens the browser. Calling it again while waiting starts over. */
  async function signIn() {
    const attempt = ++signInAttempt;
    signingIn = true;
    signInError = null;
    try {
      await social.signIn("discord");
    } catch (err) {
      // Starting over rejects the earlier call; only the latest one counts.
      if (attempt === signInAttempt) signInError = (err as Error).message;
    } finally {
      if (attempt === signInAttempt) signingIn = false;
    }
  }
</script>

<aside class="flex min-h-0 flex-col border-r border-white/2.5 bg-(--bg-side)">
  <nav aria-label="Main" class="grid gap-px px-2.5 pt-3 pb-1.5">
    {#each links as link (link.label)}
      {@const active = isActive(link.view)}
      <button
        onclick={() => go(link.view)}
        aria-current={active ? "page" : undefined}
        class="flex h-8.5 w-full items-center gap-2.5 rounded-md px-2.5 text-left transition-colors
          {active
          ? 'bg-(--bg-raise-2) text-(--text-primary)'
          : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
      >
        <Icon name={link.icon} />
        {link.label}
        {#if badge(link.view)}
          <span
            class="ml-auto rounded-full bg-(--accent) px-1.5 text-[11px] leading-4.25 font-semibold text-(--accent-ink) tabular-nums"
          >
            {badge(link.view)}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <div
    class="flex items-center justify-between px-5 pt-4 pb-1.5 text-[11.5px] font-semibold text-(--text-faint)"
  >
    <span>Instances</span>
    <button
      onclick={() => openDialog({ kind: "track" })}
      aria-label="Track an instance"
      title="Track an instance"
      class="grid rounded p-0.5 hover:bg-(--bg-hover) hover:text-(--text-primary)"
    >
      <Icon name="plus" class="size-3.5" />
    </button>
  </div>

  <!-- grid-cols-[minmax(0,1fr)]: a grid column otherwise grows to fit its widest item, so
       long names would push the list sideways instead of truncating. -->
  <div
    class="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)] content-start gap-px overflow-y-auto px-2.5 pb-2.5"
  >
    {#each instances.list as inst (inst.id)}
      {@const view = { page: "instance", id: inst.id } as const}
      {@const active = isActive(view)}
      <button
        onclick={() => go(view)}
        aria-current={active ? "page" : undefined}
        class="flex min-h-10 w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition-colors
          {active
          ? 'bg-(--bg-raise-2) text-(--text-primary)'
          : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
      >
        <ContentIcon
          src={inst.icon}
          seed={inst.id}
          class="size-6 rounded {inst.status === 'missing'
            ? 'opacity-40 grayscale'
            : ''}"
        />
        <span class="min-w-0 flex-1">
          <b class="block truncate text-[13px] font-medium">{inst.name}</b>
          <small
            class="block truncate text-[11px] {inst.status === 'missing'
              ? 'text-(--warn)'
              : 'text-(--text-faint)'}"
          >
            {inst.status === "missing"
              ? "Folder missing"
              : inst.status === "syncing"
                ? "Updating…"
                : inst.launcher.name}
          </small>
        </span>
        {#if needsYou(inst)}
          <span
            class="size-1.75 shrink-0 rounded-full bg-(--warn)"
            title="Needs your attention"
          ></span>
        {/if}
      </button>
    {:else}
      {#if instances.loaded}
        <p class="px-2.5 py-1.5 text-xs text-(--text-faint)">
          Nothing tracked yet.
        </p>
      {/if}
    {/each}
  </div>

  {#if social.session?.user}
    <button
      onclick={() => openDialog({ kind: "account" })}
      aria-label="Account and settings"
      class="mx-2.5 mb-2.5 flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-left hover:bg-(--bg-hover)"
    >
      <Head
        user={social.session.user}
        size={30}
        presence={social.session.online && !social.session.user.appearOffline
          ? "online"
          : null}
        ring="var(--bg-side)"
      />
      <span class="min-w-0 flex-1">
        <b class="block truncate text-[13px] font-medium"
          >{social.session.user.displayName}</b
        >
        <small class="block text-[11px] text-(--text-faint)"
          >{!social.session.online
            ? "Can't reach the server"
            : social.session.user.appearOffline
              ? "Appearing offline"
              : "Online"}</small
        >
      </span>
      <Icon name="more" class="size-4 text-(--text-faint)" />
    </button>
  {:else if social.loaded}
    <div class="px-5 pt-3 pb-4">
      <button
        onclick={signIn}
        disabled={signingIn}
        class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg-raise-2) font-medium hover:bg-[#252b33] disabled:opacity-50"
      >
        {signingIn ? "Waiting for your browser…" : "Sign in with Discord"}
      </button>
      {#if signingIn}
        <p class="mt-2 text-center text-[11.5px] text-(--text-faint)">
          Finish signing in there.
          <button onclick={signIn} class="underline hover:text-(--text-primary)"
            >Start again</button
          >
        </p>
      {:else if signInError}
        <p class="mt-2 text-center text-[11.5px] text-(--danger)" role="alert">
          {signInError}
        </p>
      {/if}
      <p
        class="mt-2.5 text-center text-[11px] leading-snug text-(--text-faint)"
      >
        By signing in you agree to the
        <button
          type="button"
          class="underline hover:text-(--text-primary)"
          onclick={() => openLegal("terms")}>Terms</button
        >
        and
        <button
          type="button"
          class="underline hover:text-(--text-primary)"
          onclick={() => openLegal("privacy")}>Privacy Policy</button
        >.
      </p>
    </div>
  {/if}
</aside>
