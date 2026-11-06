<script lang="ts">
  import { tick } from "svelte";
  import { api } from "$lib/api";
  import {
    fileSize,
    KIND,
    KINDS,
    nameList,
    plural,
    timeAgo,
    versionLabel,
  } from "$lib/format";
  import { fuzzyFilter } from "$lib/fuzzy";
  import { openDialog } from "$lib/dialogs.svelte";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { showDone } from "$lib/toasts.svelte";
  import { instances, social } from "$lib/stores";
  import type {
    ContentKind,
    Id,
    InstanceSummary,
    Launcher,
    LauncherKind,
  } from "$lib/types";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";
  import IconPile from "../IconPile.svelte";
  import LauncherIcon from "../LauncherIcon.svelte";
  import PackCover from "../PackCover.svelte";
  import SearchBox from "../SearchBox.svelte";
  import Switch from "../Switch.svelte";

  let {
    sharedInstanceId,
    onclose,
  }: { sharedInstanceId: Id; onclose: () => void } = $props();

  const invite = $derived(social.shared.find((s) => s.id === sharedInstanceId));
  // Look again: a launcher installed while Atlas runs is offered too.
  const launchers = resource(() =>
    api
      .detectLaunchers()
      .catch(() => null)
      .then(() => api.listLaunchers()),
  );
  const found = $derived(launchers.data ?? []);
  const canMake = (l: Launcher) =>
    !!invite && l.creatableLoaders.includes(invite.loader);
  // Launchers that can make a new instance on the invite's loader.
  const creatable = $derived(found.filter(canMake));

  // Instances already tracked here that aren't shared: the files can go into one of those.
  const spare = $derived(
    instances.list.filter((i) => !i.sync && i.status !== "missing"),
  );
  // Instances already on the right versions; the rest can't take it, so aren't offered.
  const usable = $derived(
    spare
      .filter((i) => !mismatch(i))
      .sort((a, b) =>
        (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? ""),
      ),
  );

  // Where the files go: a new instance in a launcher, or an instance you already have.
  type Target = { kind: "launcher" | "instance"; id: Id };

  // What the user clicked, kept apart from the default: a writable $derived went back to
  // the first launcher whenever the lists reloaded, quietly undoing their choice.
  let chosen = $state<Target | null>(null);
  const target = $derived<Target | null>(
    chosen ??
      (creatable[0]
        ? { kind: "launcher", id: creatable[0].id }
        : usable[0]
          ? { kind: "instance", id: usable[0].id }
          : null),
  );
  const picked = (kind: Target["kind"], id: Id) =>
    target?.kind === kind && target.id === id;

  // Optional extras the user ticks (none to begin with).
  let optional = $state<string[]>([]);

  // Starts where Settings says (on unless changed there); the friend can switch it here.
  let autoUpdate = $state(true);
  const settings = resource(() => api.getAppSettings());
  $effect(() => {
    if (settings.data) autoUpdate = settings.data.autoUpdate;
  });

  let submitting = $state(false);
  let error = $state<string | null>(null);

  const canSubmit = $derived(!submitting && !!invite && !!target);
  const pickedLauncher = $derived(
    target?.kind === "launcher"
      ? found.find((l) => l.id === target?.id)
      : undefined,
  );
  const pickedInstance = $derived(
    target?.kind === "instance"
      ? usable.find((i) => i.id === target?.id)
      : undefined,
  );

  /** The Minecraft Launcher calls them profiles. */
  const newWhat = (l: Launcher) =>
    l.kind === "official" ? "profile" : "instance";
  const loaderName = $derived(invite ? versionLabel(invite.loader, null) : "");
  const versions = $derived(
    invite ? versionLabel(invite.loader, invite.minecraftVersion) : "",
  );

  /** "the 12 items"; "the files" for a pack of only config files, or while it loads. */
  const theItems = () =>
    invite?.contentCount
      ? `the ${plural(invite.contentCount, "item")}`
      : "the files";

  // What each launcher asks of you on top (see launchers/mod.rs: create_blocker).
  const ALSO: Partial<Record<LauncherKind, string>> = {
    modrinth: "The Modrinth App asks you to confirm it.",
    curseforge: "Close CurseForge first.",
    official: "Close the launcher first.",
  };
  const inName = (l: Launcher) =>
    l.kind === "modrinth" || l.kind === "official" ? `the ${l.name}` : l.name;

  // The button says what it will do.
  const action = $derived(
    pickedLauncher
      ? `Add to ${pickedLauncher.name}`
      : pickedInstance
        ? `Add to ${pickedInstance.name}`
        : "Add pack",
  );
  // Files someone uploaded. A server from before 0.4.1 sends only their names.
  const uploaded = $derived(
    !invite
      ? []
      : invite.uploaded?.length
        ? invite.uploaded
        : invite.uploadedFiles.map((name) => ({
            name,
            fileName: null as string | null,
            sizeBytes: null as number | null,
            icon: null as string | null,
          })),
  );
  const uploadedBytes = $derived(
    uploaded.reduce((n, f) => n + (f.sizeBytes ?? 0), 0),
  );

  const extras = $derived(invite?.optionalItems ?? []);
  const extrasBytes = $derived(
    extras
      .filter((o) => optional.includes(o.key))
      .reduce((n, o) => n + o.sizeBytes, 0),
  );

  // Nothing found that can take it: say what Atlas found, and how to get somewhere.
  const ready = $derived(!!invite && !launchers.loading && !launchers.error);
  const noLauncher = $derived(ready && !found.length && !usable.length);
  const nowhere = $derived(
    ready && !!found.length && !creatable.length && !usable.length,
  );

  /** A note when an existing instance is on other versions than the shared one. */
  function mismatch(i: InstanceSummary) {
    if (!invite) return null;
    const same =
      i.minecraftVersion === invite.minecraftVersion &&
      i.loader === invite.loader;
    return same
      ? null
      : `It's on ${versionLabel(i.loader, i.minecraftVersion)}; this needs ${versionLabel(invite.loader, invite.minecraftVersion)}`;
  }
  const where = (i: InstanceSummary) =>
    i.launcher.kind === "folder" ? "A folder you added" : i.launcher.name;
  const played = (i: InstanceSummary) =>
    i.lastPlayedAt ? `played ${timeAgo(i.lastPlayedAt)}` : "never played";

  // ── Long lists open as a page inside the dialog; Back returns to the same place ──
  type View = "main" | "extras" | "files" | "dest";
  let view = $state<View>("main");
  let mainEl = $state<HTMLElement>();
  let mainScroll = 0;
  let q = $state({ extras: "", files: "", dest: "" });
  let extrasKind = $state<ContentKind | "all">("all");

  async function open(next: View) {
    mainScroll = mainEl?.scrollTop ?? 0;
    view = next;
    await tick();
    document
      .querySelector<HTMLInputElement>("dialog [data-page] input")
      ?.focus();
  }
  async function back() {
    const from = view;
    view = "main";
    await tick();
    if (mainEl) mainEl.scrollTop = mainScroll;
    document.querySelector<HTMLElement>(`[data-opens="${from}"]`)?.focus();
  }

  const shownExtras = $derived(
    fuzzyFilter(
      extras.filter((o) => extrasKind === "all" || o.kind === extrasKind),
      q.extras,
      (o) => [o.name],
    ),
  );
  const shownFiles = $derived(
    fuzzyFilter(uploaded, q.files, (f) => [f.name, f.fileName]),
  );
  // Every launcher, with a new instance and the instances already in it; then folders
  // added by hand. A launcher that can't take the pack still shows, saying why.
  const groups = $derived.by(() => {
    const has = (i: InstanceSummary, l: { kind: string; name: string }) =>
      i.launcher.kind === l.kind && i.launcher.name === l.name;
    const out = found.map((l) => ({
      key: l.id,
      kind: l.kind,
      name: l.name,
      launcher: l as Launcher | null,
      mine: fuzzyFilter(
        usable.filter((i) => has(i, l)),
        q.dest,
        (i) => [i.name, l.name],
      ),
      offerNew:
        canMake(l) &&
        (!q.dest ||
          fuzzyFilter([l], q.dest, (x) => [x.name, "new instance"]).length > 0),
    }));
    const rest = usable.filter((i) => !found.some((l) => has(i, l)));
    if (rest.length)
      out.push({
        key: "folders",
        kind: "folder",
        name: "Added by hand",
        launcher: null,
        mine: fuzzyFilter(rest, q.dest, (i) => [i.name]),
        offerNew: false,
      });
    return out.filter(
      (g) =>
        g.mine.length ||
        g.offerNew ||
        (g.launcher && !canMake(g.launcher) && !q.dest),
    );
  });
  const otherVersions = $derived(spare.length - usable.length);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    // Enter in a list page's search lands here: that's not a yes.
    if (view !== "main" || !canSubmit || !target) return;
    submitting = true;
    error = null;
    // Named now: the lists reload below.
    const where =
      target.kind === "launcher"
        ? (pickedLauncher?.name ?? "your launcher")
        : (pickedInstance?.name ?? "your instance");
    try {
      const created =
        target.kind === "launcher"
          ? await api.acceptInvite(
              sharedInstanceId,
              target.id,
              optional,
              autoUpdate,
            )
          : await api.acceptInviteInto(
              sharedInstanceId,
              target.id,
              optional,
              autoUpdate,
            );
      await Promise.all([instances.load(), social.load()]);
      showDone(
        `Adding ${invite?.name ?? "it"} to ${where}. Then you're on the same mods as ${invite?.owner?.displayName ?? "the owner"}.`,
      );
      onclose();
      // Open the new instance: its sync bar shows the files arriving.
      go({ page: "instance", id: created.id });
      // The first pack: offer to start with Windows, once, so updates land before Play.
      const pc = settings.data;
      if (pc && !pc.startWithWindows && !pc.startWithWindowsOffered) {
        openDialog({
          kind: "startWithWindows",
          owner: invite?.owner?.displayName ?? null,
        });
      }
    } catch (err) {
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }

  const sharer = $derived(invite?.invitedBy ?? invite?.owner ?? null);
  const PAGES: Record<Exclude<View, "main">, () => [string, string]> = {
    extras: () => [
      "Optional extras",
      "Only downloaded if you tick them. You can change this later.",
    ],
    files: () => [
      `Files from ${invite?.owner?.displayName ?? "the owner"}'s PC`,
      "They aren't from Modrinth or CurseForge, so nobody has checked them.",
    ],
    dest: () => [
      "Install into",
      `A new instance, or one you already have on ${versions}.`,
    ],
  };
</script>

{#snippet pageHead(titleId: string, close: () => void)}
  {@const [title, sub] = PAGES[view as Exclude<View, "main">]()}
  <div class="flex shrink-0 items-start gap-2.5 pt-4 pr-4 pb-2.5 pl-3">
    <button
      type="button"
      aria-label="Back"
      onclick={back}
      class="grid size-8 shrink-0 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
    >
      <Icon name="back" class="size-4" />
    </button>
    <div class="min-w-0 flex-1 pt-1">
      <h2
        id={titleId}
        class="font-(family-name:--font-display) text-base font-semibold"
      >
        {title}
      </h2>
      <p class="mt-0.5 text-[13px] text-(--text-muted)">{sub}</p>
    </div>
    <button
      type="button"
      aria-label="Close"
      onclick={close}
      class="grid size-8 shrink-0 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
    >
      <Icon name="close" class="size-3.5" />
    </button>
  </div>
{/snippet}

<!-- One row of a list: the item's picture, its name and what it is, then the control. -->
{#snippet radioRow(on: boolean, onpick: () => void, label: string)}
  <input
    type="radio"
    name="target"
    checked={on}
    onchange={onpick}
    aria-label={label}
    class="sr-only"
  />
{/snippet}

{#snippet instanceRow(i: InstanceSummary, sub: string)}
  {@const on = picked("instance", i.id)}
  <label
    class="flex min-h-13 cursor-pointer items-center gap-3 px-3 py-1.5 has-focus-visible:outline-2 has-focus-visible:-outline-offset-2 has-focus-visible:outline-(--accent)
      {on ? 'bg-(--accent)/7' : 'hover:bg-(--bg-hover)'}"
  >
    {@render radioRow(
      on,
      () => (chosen = { kind: "instance", id: i.id }),
      `${i.name}, ${sub}`,
    )}
    <ContentIcon src={i.icon} seed={i.id} class="size-8 rounded-md" />
    <span class="min-w-0 flex-1">
      <b class="block truncate font-medium" title={i.name}>{i.name}</b>
      <small class="block truncate text-[13px] text-(--text-faint)">{sub}</small
      >
    </span>
    <span
      class="grid size-4.5 shrink-0 place-items-center rounded-full border-[1.5px] {on
        ? 'border-(--accent)'
        : 'border-(--line-2)'}"
    >
      {#if on}<span class="size-2 rounded-full bg-(--accent)"></span>{/if}
    </span>
  </label>
{/snippet}

{#snippet extraRow(o: (typeof extras)[number])}
  {@const on = optional.includes(o.key)}
  <label
    class="flex min-h-13 cursor-pointer items-center gap-3 px-3 py-1.5 has-focus-visible:outline-2 has-focus-visible:-outline-offset-2 has-focus-visible:outline-(--accent)
      {on ? 'bg-(--accent)/7' : 'hover:bg-(--bg-hover)'}"
  >
    <input
      type="checkbox"
      bind:group={optional}
      value={o.key}
      class="size-4 shrink-0 accent-(--accent)"
    />
    <ContentIcon src={o.icon} seed={o.key} class="size-8 rounded-md" />
    <span class="min-w-0 flex-1">
      <b class="block truncate font-medium" title={o.name}>{o.name}</b>
      <small class="block text-[13px] text-(--text-faint)">{KIND[o.kind]}</small
      >
    </span>
    <span class="shrink-0 text-[13px] text-(--text-muted) tabular-nums"
      >{fileSize(o.sizeBytes)}</span
    >
  </label>
{/snippet}

{#snippet fileRow(f: (typeof uploaded)[number])}
  <li class="flex min-h-13 items-center gap-3 px-3 py-1.5">
    <ContentIcon
      src={f.icon}
      seed={f.fileName ?? f.name}
      class="size-8 rounded-md"
    />
    <span class="min-w-0 flex-1">
      <b class="block truncate font-medium" title={f.name}>{f.name}</b>
      {#if f.fileName}<small
          class="block truncate text-[13px] text-(--text-faint)"
          title={f.fileName}>{f.fileName}</small
        >{/if}
    </span>
    {#if f.sizeBytes != null}<span
        class="shrink-0 text-[13px] text-(--text-muted) tabular-nums"
        >{fileSize(f.sizeBytes)}</span
      >{/if}
  </li>
{/snippet}

<!-- A long list as one row: its first icons stacked, a few names, and where it opens. -->
{#snippet summary(
  opens: View,
  label: string,
  icons: { src: string | null; seed: string }[],
  title: string,
  sub: string,
  more: string,
  ring = "var(--bg-raise)",
)}
  <button
    type="button"
    data-opens={opens}
    aria-label={label}
    onclick={() => open(opens)}
    class="flex min-h-15 w-full items-center gap-3 rounded-[10px] border border-(--line)/60 px-3 py-2.5 text-left hover:border-(--line) hover:bg-(--bg-hover)"
  >
    <IconPile items={icons} {ring} />
    <span class="min-w-0 flex-1">
      <b class="block truncate font-medium">{title}</b>
      <small class="block text-[13px] text-(--text-faint)">{sub}</small>
    </span>
    <span
      class="flex shrink-0 items-center gap-1 text-[13px] font-semibold text-(--accent)"
      >{more}<Icon name="chev" class="size-3" /></span
    >
  </button>
{/snippet}

<Dialog
  title={invite ? `Add ${invite.name}` : "Shared pack"}
  wide
  {onclose}
  onsubmit={submit}
  onescape={() => {
    if (view === "main") return false;
    back();
    return true;
  }}
  bodyClass="flex min-h-0 flex-1 flex-col"
>
  {#snippet head({ titleId, close })}
    {#if view !== "main"}
      {@render pageHead(titleId, close)}
    {:else}
      <div class="relative shrink-0">
        <PackCover seed={sharedInstanceId} src={invite?.icon ?? null} />
        <button
          type="button"
          aria-label="Close"
          onclick={close}
          class="absolute top-3 right-3 grid size-8 place-items-center rounded-md bg-(--bg)/80 text-(--text-primary) hover:bg-(--bg)"
        >
          <Icon name="close" class="size-3.5" />
        </button>
      </div>
      <div
        class="relative -mt-3.5 flex shrink-0 items-start gap-3.5 px-5.5 pb-1"
      >
        <span
          class="shrink-0 rounded-[10px] shadow-[0_0_0_3px_var(--bg-raise)]"
        >
          <ContentIcon
            src={invite?.icon ?? null}
            seed={sharedInstanceId}
            class="size-16 rounded-[10px]"
          />
        </span>
        <div class="min-w-0 pt-4.5">
          <h2
            id={titleId}
            class="line-clamp-2 font-(family-name:--font-display) text-[20px] leading-tight font-semibold"
            title={invite?.name}
          >
            {invite?.name ?? "Shared pack"}
          </h2>
          {#if invite}
            <div
              class="mt-1 flex flex-wrap items-center gap-x-1.75 gap-y-1.5 text-[13px] text-(--text-muted)"
            >
              {#if sharer}
                <Head user={sharer} size={20} />
                <span
                  >{sharer.displayName} shared it {timeAgo(
                    invite.invitedAt ?? invite.updatedAt,
                  )}</span
                >
              {/if}
              <span
                class="ml-1.5 inline-flex h-6 items-center rounded-full bg-(--bg-raise-2) px-2.25"
                >{versions}</span
              >
              <span
                class="inline-flex h-6 items-center rounded-full bg-(--bg-raise-2) px-2.25"
                >{plural(invite.contentCount, "item")}</span
              >
            </div>
          {/if}
        </div>
      </div>
    {/if}
  {/snippet}

  {#if !invite}
    <p class="px-5.5 py-6 text-[13px] text-(--text-muted)">
      This pack isn't available anymore.
    </p>
  {:else if submitting}
    <!-- While it works: say what's happening and what, if anything, to do. -->
    <div class="px-5.5 pt-4 pb-5">
      <div
        class="flex items-start gap-3.5 rounded-lg bg-(--bg-raise-2)/50 px-4 py-4"
      >
        <Icon
          name="refresh"
          class="mt-0.5 size-5 shrink-0 animate-spin text-(--accent)"
        />
        <div class="text-[13px]">
          {#if pickedLauncher?.kind === "modrinth"}
            <b class="block text-[14px] font-semibold"
              >Confirm in the Modrinth App</b
            >
            <p class="mt-1 text-(--text-muted)">
              It has opened and is asking whether to add “{invite.name}”.
              Confirm there, and Atlas adds {theItems()} by itself.
            </p>
          {:else if pickedLauncher}
            <b class="block text-[14px] font-semibold"
              >Creating the {newWhat(pickedLauncher)} in {pickedLauncher.name}…</b
            >
            <p class="mt-1 text-(--text-muted)">
              Then Atlas adds {theItems()}.
            </p>
          {:else}
            <b class="block text-[14px] font-semibold"
              >Linking {pickedInstance?.name ?? "the instance"}…</b
            >
            <p class="mt-1 text-(--text-muted)">
              Then Atlas adds {theItems()}.
            </p>
          {/if}
        </div>
      </div>
    </div>
  {:else if view === "extras"}
    <div data-page class="flex h-[min(520px,calc(100vh-260px))] flex-col">
      <div class="flex shrink-0 flex-col gap-2 px-5.5 pb-2.5">
        <SearchBox
          bind:value={q.extras}
          placeholder="Search {plural(extras.length, 'extra')}"
        />
        <div class="flex flex-wrap items-center gap-2">
          <div class="flex flex-wrap gap-1" role="group" aria-label="Kind">
            {#each [["all", "All"], ...(Object.keys(KINDS) as ContentKind[])
                .filter((k) => extras.some((o) => o.kind === k))
                .map((k) => [k, KINDS[k]])] as [k, label] (k)}
              <button
                type="button"
                aria-pressed={extrasKind === k}
                onclick={() => (extrasKind = k as ContentKind | "all")}
                class="h-7 rounded-full px-2.75 text-[13px] font-medium {extrasKind ===
                k
                  ? 'bg-(--accent)/15 text-(--accent)'
                  : 'text-(--text-muted) hover:bg-(--bg-hover)'}"
              >
                {label}
                <span class="ml-0.5 font-normal opacity-70"
                  >{k === "all"
                    ? extras.length
                    : extras.filter((o) => o.kind === k).length}</span
                >
              </button>
            {/each}
          </div>
          <span class="ml-auto flex gap-0.5">
            <button
              type="button"
              disabled={!shownExtras.length}
              onclick={() =>
                (optional = [
                  ...new Set([...optional, ...shownExtras.map((o) => o.key)]),
                ])}
              class="h-7 rounded-md px-2 text-[13px] font-medium text-(--accent) hover:bg-(--bg-hover) disabled:opacity-50"
              >Tick all shown</button
            >
            <button
              type="button"
              disabled={!optional.length}
              onclick={() => (optional = [])}
              class="h-7 rounded-md px-2 text-[13px] font-medium text-(--text-muted) hover:bg-(--bg-hover) disabled:opacity-50"
              >Untick all</button
            >
          </span>
        </div>
      </div>
      <div
        class="mx-5.5 mb-3 min-h-0 flex-1 overflow-y-auto rounded-[10px] border border-(--line)/60 [&>*+*]:border-t [&>*+*]:border-(--line)/60"
        role="group"
        aria-label="Optional extras"
      >
        {#each shownExtras as o (o.key)}
          {@render extraRow(o)}
        {:else}
          <p class="px-4 py-7 text-center text-[13px] text-(--text-muted)">
            No extras match “{q.extras}”.
          </p>
        {/each}
      </div>
    </div>
  {:else if view === "files"}
    <div data-page class="flex h-[min(520px,calc(100vh-260px))] flex-col">
      <div class="shrink-0 px-5.5 pb-2.5">
        <SearchBox
          bind:value={q.files}
          placeholder="Search {plural(uploaded.length, 'file')}"
        />
      </div>
      <ul
        class="mx-5.5 mb-3 min-h-0 flex-1 overflow-y-auto rounded-[10px] border border-(--line)/60 [&>*+*]:border-t [&>*+*]:border-(--line)/60"
      >
        {#each shownFiles as f, i (i)}
          {@render fileRow(f)}
        {:else}
          <li class="px-4 py-7 text-center text-[13px] text-(--text-muted)">
            No files match “{q.files}”.
          </li>
        {/each}
      </ul>
    </div>
  {:else if view === "dest"}
    <div data-page class="flex h-[min(520px,calc(100vh-260px))] flex-col">
      <div class="shrink-0 px-5.5 pb-2.5">
        <SearchBox
          bind:value={q.dest}
          placeholder="Search {plural(usable.length, 'instance')}"
        />
      </div>
      <div
        class="mx-5.5 mb-3 min-h-0 flex-1 overflow-y-auto rounded-[10px] border border-(--line)/60"
        role="radiogroup"
        aria-label="Install into"
      >
        {#each groups as g, gi (g.key)}
          <div
            class="flex items-center gap-2 px-3 pt-3 pb-1.5 text-[13px] font-semibold text-(--text-muted) {gi
              ? 'mt-1 border-t border-(--line)/60'
              : ''}"
          >
            <LauncherIcon kind={g.kind as LauncherKind} class="size-5" />
            {g.name}
            {#if g.launcher && !canMake(g.launcher)}<span
                class="font-normal text-(--text-faint)"
                >Can't make {loaderName} instances</span
              >{/if}
          </div>
          {#if g.offerNew && g.launcher}
            {@const l = g.launcher}
            {@const on = picked("launcher", l.id)}
            <label
              class="flex min-h-13 cursor-pointer items-center gap-3 px-3 py-1.5 has-focus-visible:outline-2 has-focus-visible:-outline-offset-2 has-focus-visible:outline-(--accent)
                {on ? 'bg-(--accent)/7' : 'hover:bg-(--bg-hover)'}"
            >
              {@render radioRow(
                on,
                () => (chosen = { kind: "launcher", id: l.id }),
                `New ${newWhat(l)} in ${l.name}`,
              )}
              <span
                class="grid size-8 shrink-0 place-items-center rounded-md border-[1.5px] border-dashed border-(--line-2) text-(--text-muted)"
                ><Icon name="plus" class="size-4" /></span
              >
              <span class="min-w-0 flex-1">
                <b class="block font-medium">New {newWhat(l)}</b>
                <small class="block text-[13px] text-(--text-faint)"
                  >Made for this pack</small
                >
              </span>
              <span
                class="grid size-4.5 shrink-0 place-items-center rounded-full border-[1.5px] {on
                  ? 'border-(--accent)'
                  : 'border-(--line-2)'}"
              >
                {#if on}<span class="size-2 rounded-full bg-(--accent)"
                  ></span>{/if}
              </span>
            </label>
          {/if}
          {#each g.mine as i (i.id)}
            {@render instanceRow(
              i,
              g.launcher ? played(i) : `${where(i)}, ${played(i)}`,
            )}
          {/each}
        {:else}
          <p class="px-4 py-7 text-center text-[13px] text-(--text-muted)">
            No instances match “{q.dest}”.
          </p>
        {/each}
        {#if otherVersions > 0 && !q.dest}
          <p
            class="border-t border-(--line)/60 px-3 py-3 text-[13px] text-(--text-faint)"
          >
            {plural(otherVersions, "instance")} on other versions
            {otherVersions === 1 ? "isn't" : "aren't"} listed.
          </p>
        {/if}
      </div>
    </div>
  {:else}
    <div
      bind:this={mainEl}
      class="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-5.5 pt-4 pb-5"
    >
      {#if uploaded.length}
        {@const one = uploaded.length === 1}
        {@const owner = invite.owner?.displayName ?? "the owner"}
        <!-- First, so it's read before choosing where the pack goes. -->
        <section
          class="grid gap-2.5 rounded-[10px] border border-(--warn)/30 bg-(--warn)/7 p-3"
          aria-label="Files nobody has checked"
        >
          <div class="flex items-start gap-2.5">
            <Icon name="warn" class="mt-0.5 size-4 shrink-0 text-(--warn)" />
            <div class="min-w-0 text-[13px] text-(--text-muted)">
              <b class="block text-[14px] font-semibold text-(--text-primary)"
                >{one ? "One file came" : `${uploaded.length} files came`} from {owner}'s
                own PC</b
              >
              {one ? "It isn't" : "They aren't"} from Modrinth or CurseForge, so nobody
              has checked {one ? "it" : "them"}. Only add the pack if you trust {owner}.
            </div>
          </div>
          {#if one}
            <ul class="rounded-[10px] border border-(--warn)/20 bg-[#171a1c]">
              {@render fileRow(uploaded[0])}
            </ul>
          {:else}
            {@render summary(
              "files",
              `${uploaded.length === 2 ? "See both files" : `See all ${uploaded.length} files`} from ${owner}'s PC`,
              uploaded.map((f) => ({ src: f.icon, seed: f.fileName ?? f.name })),
              nameList(uploaded.map((f) => f.name)),
              uploadedBytes ? `${fileSize(uploadedBytes)} in all` : "",
              uploaded.length === 2 ? "See both" : `See all ${uploaded.length}`,
              "#171a1c",
            )}
          {/if}
        </section>
      {/if}

      <section class="grid gap-2.5">
        <h3 class="text-[14px] font-semibold">Install into</h3>
        {#if launchers.loading}
          <div
            class="grid grid-cols-[repeat(auto-fill,minmax(124px,1fr))] gap-2"
            aria-busy="true"
          >
            {#each [0, 1, 2, 3] as n (n)}
              <div
                class="h-24 animate-pulse rounded-[10px] bg-(--bg-raise-2)"
              ></div>
            {/each}
          </div>
          <p class="text-[13px] text-(--text-muted)">Looking for launchers…</p>
        {:else if launchers.error}
          <p class="text-[13px] text-(--danger)">
            {launchers.error}
            <button
              type="button"
              class="ml-1 underline"
              onclick={launchers.reload}>Try again</button
            >
          </p>
        {:else if noLauncher}
          <div
            class="grid justify-items-start gap-2 rounded-[10px] border border-dashed border-(--line) p-3.5 text-[13px] text-(--text-muted)"
          >
            <Icon name="search" class="size-4.5 text-(--text-faint)" />
            <b class="text-[14px] font-semibold text-(--text-primary)"
              >Atlas didn't find a launcher</b
            >
            <span
              >It looks for these in their usual folders. Install one, or add
              one Atlas missed, like a portable Prism.</span
            >
            <ul
              class="flex flex-wrap gap-1.5"
              aria-label="Launchers Atlas looks for"
            >
              {#each [["prism", "Prism Launcher"], ["multimc", "MultiMC"], ["modrinth", "Modrinth App"], ["curseforge", "CurseForge"], ["official", "Minecraft Launcher"]] as [k, n] (k)}
                <li
                  class="inline-flex h-7 items-center gap-1.75 rounded-full bg-(--bg-raise-2) pr-2.5 pl-1"
                >
                  <LauncherIcon kind={k as LauncherKind} class="size-5" />{n}
                </li>
              {/each}
            </ul>
            <span class="mt-0.5 flex flex-wrap gap-2">
              <Button size="sm" icon="refresh" onclick={launchers.reload}
                >Look again</Button
              >
              <Button size="sm" onclick={() => openDialog({ kind: "track" })}
                >Add a launcher</Button
              >
            </span>
          </div>
        {:else}
          {#if found.length}
            <div
              class="grid grid-cols-[repeat(auto-fill,minmax(124px,1fr))] gap-2"
              role="radiogroup"
              aria-label="New instance in"
            >
              {#each found as l (l.id)}
                {@const on = picked("launcher", l.id)}
                {@const ok = canMake(l)}
                <label
                  class="relative flex flex-col items-center gap-2 rounded-[10px] border px-2 pt-3.5 pb-3 text-center has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-(--accent)
                    {on
                    ? 'border-(--accent) bg-(--accent)/7 shadow-[inset_0_0_0_1px_var(--accent)]'
                    : 'border-(--line)'}
                    {ok
                    ? 'cursor-pointer hover:border-(--line-2) hover:bg-(--bg-hover)'
                    : 'cursor-not-allowed'}"
                >
                  <input
                    type="radio"
                    name="target"
                    checked={on}
                    disabled={!ok}
                    onchange={() => (chosen = { kind: "launcher", id: l.id })}
                    class="sr-only"
                  />
                  <LauncherIcon
                    kind={l.kind}
                    class="size-9 {ok ? '' : 'opacity-50'}"
                  />
                  <b
                    class="block text-[13px] leading-snug font-medium {ok
                      ? ''
                      : 'text-(--text-muted)'}">{l.name}</b
                  >
                  {#if !ok}<small
                      class="-mt-1 block text-[13px] leading-snug text-(--text-faint)"
                      >Can't make {loaderName} instances</small
                    >{/if}
                  {#if on}
                    <span
                      class="absolute top-1.5 right-1.5 grid size-4.5 place-items-center rounded-full bg-(--accent) text-(--accent-ink)"
                      ><Icon name="check" class="size-2.75" /></span
                    >
                  {/if}
                </label>
              {/each}
            </div>
          {:else}
            <p class="-mt-1 text-[13px] text-(--text-faint)">
              Atlas can't make new instances in folders you added by hand, so it
              adds the pack to one you already have.
            </p>
          {/if}

          {#if nowhere}
            <div
              class="grid justify-items-start gap-2 rounded-[10px] border border-dashed border-(--line) p-3.5 text-[13px] text-(--text-muted)"
            >
              <b class="text-[14px] font-semibold text-(--text-primary)"
                >Nowhere to add it yet</b
              >
              <span
                >{nameList(found.map((l) => l.name))}
                can't make {loaderName} instances, and none of your instances is on
                {versions}. Add a launcher that can, or an instance that's on {versions}.</span
              >
              <span class="mt-0.5 flex flex-wrap gap-2">
                <Button size="sm" onclick={() => openDialog({ kind: "track" })}
                  >Add a launcher or instance</Button
                >
              </span>
            </div>
          {:else if !found.length}
            <!-- Only folders added by hand: your instances are the choice itself. -->
            <div
              class="overflow-hidden rounded-[10px] border border-(--line)/60 [&>*+*]:border-t [&>*+*]:border-(--line)/60"
              role="radiogroup"
              aria-label="Your instances"
            >
              {#each usable.slice(0, 3) as i (i.id)}
                {@render instanceRow(i, `${where(i)}, ${played(i)}`)}
              {/each}
            </div>
            {#if usable.length > 3}
              <button
                type="button"
                data-opens="dest"
                onclick={() => open("dest")}
                class="-mt-0.5 flex min-h-10 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
                ><span class="flex-1">See all {usable.length}</span><Icon
                  name="chev"
                  class="size-3"
                /></button
              >
            {/if}
          {:else if pickedInstance}
            <div
              class="flex min-h-13 items-center gap-3 rounded-[10px] border border-(--accent) bg-(--accent)/7 px-3 py-1.5 shadow-[inset_0_0_0_1px_var(--accent)]"
            >
              <ContentIcon
                src={pickedInstance.icon}
                seed={pickedInstance.id}
                class="size-8 rounded-md"
              />
              <span class="min-w-0 flex-1">
                <b class="block truncate font-medium">{pickedInstance.name}</b>
                <small class="block truncate text-[13px] text-(--text-faint)"
                  >{where(pickedInstance)}, {played(pickedInstance)}</small
                >
              </span>
              <Button
                size="sm"
                data-opens="dest"
                aria-label="Change where the pack goes"
                onclick={() => open("dest")}>Change</Button
              >
            </div>
          {/if}

          <!-- What the choice will do, said right under it. -->
          {#if pickedLauncher}
            <p class="text-[13px] text-(--text-muted)" aria-live="polite">
              Creates a new {newWhat(pickedLauncher)} in {inName(
                pickedLauncher,
              )} with
              {theItems()}. {ALSO[pickedLauncher.kind] ?? ""}
            </p>
          {:else if pickedInstance}
            <p class="text-[13px] text-(--text-muted)" aria-live="polite">
              Adds {theItems()} to
              <b class="font-semibold text-(--text-primary)"
                >{pickedInstance.name}</b
              >. Your own files there stay.
            </p>
          {/if}

          {#if found.length && usable.length && !pickedInstance}
            <button
              type="button"
              data-opens="dest"
              onclick={() => open("dest")}
              aria-label="Use an instance you already have, {usable.length} to choose from"
              class="-mt-0.5 flex min-h-10 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
            >
              <Icon name="folder" class="size-4" />
              <span class="flex-1">Use an instance you already have</span>
              <span
                class="grid h-5 min-w-5 place-items-center rounded-full bg-(--bg-raise-2) px-1.75 tabular-nums"
                >{usable.length}</span
              >
              <Icon name="chev" class="size-3" />
            </button>
          {/if}
        {/if}
      </section>

      {#if extras.length}
        {@const ticked = extras.filter((o) => optional.includes(o.key))}
        {@const note = ticked.length
          ? `${ticked.length} of ${extras.length} ticked, ${fileSize(extrasBytes)}`
          : "None ticked"}
        <section class="grid gap-2.5">
          <div class="flex items-baseline gap-2.5">
            <h3 class="text-[14px] font-semibold">Optional extras</h3>
            <span class="ml-auto text-[13px] text-(--text-muted) tabular-nums"
              >{note}</span
            >
          </div>
          <p class="-mt-1.5 text-[13px] text-(--text-faint)">
            Only downloaded if you tick them. You can change this later.
          </p>
          {#if extras.length <= 3}
            <div
              class="overflow-hidden rounded-[10px] border border-(--line)/60 [&>*+*]:border-t [&>*+*]:border-(--line)/60"
            >
              {#each extras as o (o.key)}
                {@render extraRow(o)}
              {/each}
            </div>
          {:else}
            {@const lead = ticked.length ? ticked : extras}
            {@render summary(
              "extras",
              `${ticked.length ? "Change" : "Choose"} optional extras, ${note.toLowerCase()}`,
              lead.map((o) => ({ src: o.icon, seed: o.key })),
              nameList(lead.map((o) => o.name)),
              ticked.length
                ? "Ticked to download"
                : (Object.keys(KINDS) as ContentKind[])
                    .filter((k) => extras.some((o) => o.kind === k))
                    .map((k, i) => (i ? KINDS[k].toLowerCase() : KINDS[k]))
                    .join(", ")
                    .replace(/, ([^,]*)$/, " and $1"),
              ticked.length ? "Change" : "Choose",
            )}
          {/if}
        </section>
      {/if}

      {#if !noLauncher && !nowhere}
        <section class="flex items-center gap-3">
          <div class="min-w-0 flex-1">
            <b class="block font-medium">Keep it up to date automatically</b>
            <small class="block text-[13px] text-(--text-muted)">
              When {invite.owner?.displayName ?? "the owner"} sends an update, it's
              installed before you next play.
            </small>
          </div>
          <Switch
            checked={autoUpdate}
            label="Keep it up to date automatically"
            onchange={(next) => (autoUpdate = next)}
          />
        </section>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if view === "extras"}
      <span class="mr-auto text-[13px] text-(--text-muted) tabular-nums"
        >{optional.length
          ? `${optional.length} ticked, ${fileSize(extrasBytes)}`
          : "None ticked"}</span
      >
      <Button variant="primary" onclick={back}>Done</Button>
    {:else if view === "files"}
      <span class="mr-auto text-[13px] text-(--text-muted) tabular-nums"
        >{plural(uploaded.length, "file")}{uploadedBytes
          ? `, ${fileSize(uploadedBytes)} in all`
          : ""}</span
      >
      <Button variant="primary" onclick={back}>Done</Button>
    {:else if view === "dest"}
      <span class="mr-auto min-w-0 truncate text-[13px] text-(--text-muted)"
        >Into <b class="font-semibold text-(--text-primary)"
          >{pickedLauncher
            ? `a new ${newWhat(pickedLauncher)} in ${pickedLauncher.name}`
            : (pickedInstance?.name ?? "nothing yet")}</b
        ></span
      >
      <Button variant="primary" onclick={back}>Done</Button>
    {:else}
      {#if error}<span class="mr-auto text-[13px] text-(--danger)" role="alert"
          >{error}</span
        >{:else if invite && !submitting}<span
          class="mr-auto text-[13px] text-(--text-muted) tabular-nums"
          ><b class="font-semibold text-(--text-primary)"
            >{fileSize(invite.sizeBytes + extrasBytes)}</b
          >
          to download{extrasBytes
            ? `, including ${fileSize(extrasBytes)} of extras`
            : ""}</span
        >{/if}
      <!-- Closing doesn't stop it: the work carries on and the instance shows up when done. -->
      <Button variant="ghost" onclick={onclose}
        >{submitting ? "Close" : "Cancel"}</Button
      >
      <Button type="submit" variant="primary" disabled={!canSubmit}>
        <span class="block max-w-60 truncate"
          >{submitting ? "Working…" : action}</span
        >
      </Button>
    {/if}
  {/snippet}
</Dialog>
