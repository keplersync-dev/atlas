<script lang="ts">
  import { iconSeed } from "$lib/pixel";
  import { unfinished } from "$lib/play";
  import { api } from "$lib/api";
  import { go, isActive, type View } from "$lib/nav.svelte";
  import { openContextMenu } from "$lib/contextMenu.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { instanceItems } from "$lib/instanceMenu";
  import { instances, social } from "$lib/stores";
  import ContentIcon from "./ContentIcon.svelte";
  import Head from "./Head.svelte";
  import Icon from "./Icon.svelte";
  import { waitingOnYou } from "./InstanceCard.svelte";
  import ProviderIcon from "./ProviderIcon.svelte";
  import UpdateCard from "./UpdateCard.svelte";

  let legalError = $state<string | null>(null);

  const links: { view: View; label: string; icon: string }[] = [
    { view: { page: "home" }, label: "Home", icon: "home" },
    { view: { page: "shared" }, label: "Shared", icon: "inbox" },
    { view: { page: "friends" }, label: "Friends", icon: "users" },
  ];

  /** Count shown next to a nav link: invites on Shared, friend requests on Friends. */
  function badge(view: View) {
    if (view.page === "shared") return social.invites.length;
    if (view.page === "friends")
      return social.incomingRequests.length + social.groupInvites.length;
    return 0;
  }

  function openLegal(page: "terms" | "privacy") {
    legalError = null;
    api.openLegalPage(page).catch((e) => (legalError = (e as Error).message));
  }
</script>

<!-- This PC's settings (notifications, sound, start with Windows) need no account. -->
{#snippet settingsRow()}
  <div class="border-t border-(--line) px-2.5 py-2">
    <button
      type="button"
      onclick={() => openDialog({ kind: "account" })}
      class="flex h-8.5 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-(--text-muted) transition-colors hover:bg-(--bg-hover) hover:text-(--text-primary)"
    >
      <Icon name="settings" />
      Settings
    </button>
  </div>
{/snippet}

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
            class="ml-auto rounded-full bg-(--accent) px-1.5 text-[13px] leading-4.25 font-semibold text-(--accent-ink) tabular-nums"
          >
            {badge(link.view)}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <div
    class="flex items-center justify-between px-5 pt-4 pb-1.5 text-[13px] font-semibold text-(--text-faint)"
  >
    <span>Instances</span>
    <button
      onclick={() => openDialog({ kind: "track" })}
      aria-label="Add an instance"
      title="Add an instance"
      class="-my-1 grid size-6 place-items-center rounded hover:bg-(--bg-hover) hover:text-(--text-primary)"
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
        oncontextmenu={(e) => openContextMenu(e, instanceItems(inst))}
        aria-current={active ? "page" : undefined}
        title={inst.name}
        class="flex min-h-10 w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition-colors
          {active
          ? 'bg-(--bg-raise-2) text-(--text-primary)'
          : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
      >
        <ContentIcon
          src={inst.icon}
          seed={iconSeed(inst)}
          class="size-6 rounded {inst.status === 'missing'
            ? 'opacity-40 grayscale'
            : ''}"
        />
        <span class="min-w-0 flex-1">
          <b class="block truncate text-[13px] font-medium">{inst.name}</b>
          <!-- What needs the player is said in words, not only by the dot. -->
          <small
            class="block truncate text-[13px] {inst.status === 'missing' ||
            waitingOnYou(inst)
              ? 'text-(--warn)'
              : 'text-(--text-faint)'}"
          >
            {inst.status === "missing"
              ? "Folder missing"
              : inst.status === "syncing"
                ? "Updating…"
                : unfinished(inst)
                  ? "Not set up yet"
                  : inst.sync?.state === "update_available" ||
                      inst.sync?.state === "conflict"
                    ? "Update ready"
                    : waitingOnYou(inst)
                      ? "Changes not sent"
                      : inst.launcher.name}
          </small>
        </span>
        {#if waitingOnYou(inst)}
          <span
            class="size-1.75 shrink-0 rounded-full bg-(--warn)"
            aria-hidden="true"
          ></span>
        {/if}
      </button>
    {:else}
      {#if instances.loaded}
        <p class="px-2.5 py-1.5 text-[13px] text-(--text-faint)">None yet.</p>
      {/if}
    {/each}
  </div>

  <UpdateCard />
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
        <small class="block truncate text-[13px] text-(--text-faint)"
          >{!social.session.online
            ? "Can't reach the server"
            : social.session.user.appearOffline
              ? "Appearing offline"
              : `@${social.session.user.username}`}</small
        >
      </span>
      <Icon name="settings" class="size-4 text-(--text-faint)" />
    </button>
  {:else if social.loaded && !social.hasServer}
    <p class="px-5 pt-3 pb-1 text-center text-[13px] text-(--text-faint)">
      Friends and sharing aren't available in this copy of Atlas.
    </p>
    {@render settingsRow()}
  {:else if social.loaded}
    <div class="border-t border-(--line) px-3.5 pt-3 pb-3">
      <p class="text-[13px] font-semibold">Sign in to share with friends</p>
      <div class="mt-2 grid gap-1.5">
        <button
          onclick={() => social.signIn()}
          aria-label="Sign in with Minecraft"
          class="flex h-9 min-w-0 items-center justify-center gap-2 rounded-md border px-2 font-medium disabled:opacity-50
            {social.signingIn
            ? 'border-(--accent) bg-(--accent)/10'
            : 'border-(--line-2) bg-(--bg-raise-2) hover:bg-(--bg-hover)'}"
        >
          {#if social.signingIn}
            <Icon
              name="refresh"
              class="size-3.5 shrink-0 animate-spin text-(--accent)"
            />
            <span class="truncate">Waiting…</span>
          {:else}
            <ProviderIcon class="size-4 shrink-0" />
            <span class="truncate">Sign in with Minecraft</span>
          {/if}
        </button>
      </div>
      {#if social.signingIn}
        <p class="mt-2 text-[13px] text-(--text-muted)">
          Finish in your browser.
        </p>
        <p class="flex gap-3 text-[13px] text-(--text-muted)">
          <button
            onclick={() => social.signIn()}
            class="underline hover:text-(--text-primary)">Start again</button
          >
          <button
            onclick={() => social.cancelSignIn()}
            class="underline hover:text-(--text-primary)">Cancel</button
          >
        </p>
      {:else if social.signInError || legalError}
        <p class="mt-2 text-[13px] text-(--danger)" role="alert">
          {social.signInError ?? legalError}
        </p>
      {:else}
        <p class="mt-2 text-[13px] leading-snug text-(--text-faint)">
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
      {/if}
    </div>
    {@render settingsRow()}
  {/if}
</aside>
