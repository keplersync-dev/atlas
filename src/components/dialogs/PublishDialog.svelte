<script lang="ts">
  import { tick } from "svelte";
  import { iconSeed } from "$lib/pixel";
  import { api } from "$lib/api";
  import { openDialog } from "$lib/dialogs.svelte";
  import {
    fileSize,
    KIND,
    nameList,
    plural,
    versionLabel,
    workLabel,
  } from "$lib/format";
  import { fuzzyFilter } from "$lib/fuzzy";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import { showDone } from "$lib/toasts.svelte";
  import type { Id, PublishItem, RemovedItem, ShareMode } from "$lib/types";
  import { modeOf, publishChanges } from "$lib/publishChanges";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";
  import Menu from "../Menu.svelte";
  import SearchBox from "../SearchBox.svelte";
  import ShareModeSelect from "../ShareModeSelect.svelte";
  import { names } from "./SharePicker.svelte";

  // The update as friends will see it: your note, and every change as a row with its own
  // Required / Optional / Only me. Which config files are shared is set on the instance's
  // Sharing tab, so here they're only counted.
  let { instanceId, onclose }: { instanceId: Id; onclose: () => void } =
    $props();

  const instance = $derived(instances.byId(instanceId));
  // Reading the folder while the plan loads; uploading and saving while publishing.
  const work = $derived(instances.work[instanceId]);
  const plan = resource(() => api.preparePublish(instanceId));

  let modes = $state<Record<Id, ShareMode>>({}); // only what the user changes here
  let rights = $state(false);
  // Send was pressed without the uploads ticked: say so at the tick box.
  let rightsError = $state(false);
  let changelog = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const changes = $derived(
    plan.data ? publishChanges(plan.data, modes, null) : null,
  );
  const total = $derived(changes?.total ?? 0);

  // ── Uploads: a file over the per-file limit can't go at all, so it's its own problem
  // and doesn't count against the space ──
  const storage = $derived(plan.data?.storage);
  const sharedItems = $derived(
    plan.data?.items.filter((i) => modeOf(i, modes) !== "private") ?? [],
  );
  const toUpload = (i: PublishItem) =>
    i.needsUpload && !i.alreadyUploaded && !i.unchecked;
  const unchecked = $derived(sharedItems.filter((i) => i.unchecked));
  const tooBig = $derived(
    storage
      ? sharedItems.filter(
          (i) => toUpload(i) && i.sizeBytes > storage.maxFileBytes,
        )
      : [],
  );
  const uploads = $derived(
    storage
      ? sharedItems.filter(
          (i) => toUpload(i) && i.sizeBytes <= storage.maxFileBytes,
        )
      : [],
  );
  const adds = $derived(uploads.reduce((n, i) => n + i.sizeBytes, 0));
  const after = $derived((storage?.usedBytes ?? 0) + adds);
  const over = $derived(!!storage && after > storage.quotaBytes);

  // Who gets it.
  const people = $derived.by(() => {
    const shared = social.shared.find(
      (x) => x.id === instance?.sync?.sharedInstanceId,
    );
    const me = social.session?.user?.id;
    return (shared?.members ?? [])
      .filter((m) => m.status === "active" && m.user.id !== me)
      .map((m) => m.user);
  });
  const who = $derived(
    people.length <= 2
      ? people.map((p) => p.displayName).join(" and ")
      : `${people[0].displayName}, ${people[1].displayName} and ${people.length - 2} others`,
  );
  const version = $derived((plan.data?.baseRevision ?? 0) + 1);

  // ── The list: every change, filtered, searched, each with its choice ──
  type Row =
    | { kind: "item"; item: PublishItem; what: "new" | "updated" | "setting" }
    | { kind: "removed"; item: RemovedItem }
    | { kind: "configs" }
    | { kind: "versions" };
  // Membership is fixed when the plan loads, so a row doesn't vanish when its choice changes.
  const rows = $derived.by((): Row[] => {
    const p = plan.data;
    if (!p) return [];
    const out: Row[] = [];
    if (p.versions) out.push({ kind: "versions" });
    for (const item of p.items) {
      const was = item.publishedShareMode;
      if (item.isNew || was === null || was === "private")
        out.push({ kind: "item", item, what: "new" });
      else if (item.contentsChanged)
        out.push({ kind: "item", item, what: "updated" });
      else if (item.shareMode !== was)
        out.push({ kind: "item", item, what: "setting" });
    }
    for (const item of p.removed) out.push({ kind: "removed", item });
    if (p.filesChanged.length) out.push({ kind: "configs" });
    return out;
  });
  type Filter = "all" | "new" | "uploads" | "updated" | "removed";
  let filter = $state<Filter>("all");
  let query = $state("");
  const inFilter = (r: Row, f: Filter) =>
    f === "all" ||
    (f === "removed" && r.kind === "removed") ||
    (r.kind === "item" &&
      ((f === "new" && r.what === "new") ||
        (f === "updated" && r.what !== "new") ||
        (f === "uploads" && r.item.needsUpload && !r.item.alreadyUploaded)));
  const FILTERS: [Filter, string][] = [
    ["all", "All"],
    ["new", "New"],
    ["uploads", "Uploads"],
    ["updated", "Updated"],
    ["removed", "Removed"],
  ];
  const counts = $derived(
    Object.fromEntries(
      FILTERS.map(([f]) => [f, rows.filter((r) => inFilter(r, f)).length]),
    ) as Record<Filter, number>,
  );
  const rowName = (r: Row) =>
    r.kind === "item"
      ? r.item.name
      : r.kind === "removed"
        ? r.item.name
        : r.kind === "configs"
          ? `configs ${plan.data?.filesChanged.join(" ")}`
          : "Minecraft";
  const shown = $derived(
    fuzzyFilter(
      rows.filter((r) => inFilter(r, filter)),
      query,
      (r) => [rowName(r)],
    ),
  );
  const settable = $derived(
    shown.flatMap((r) => (r.kind === "item" ? [r.item] : [])),
  );
  const setMode = (i: PublishItem, m: ShareMode) =>
    (modes = { ...modes, [i.contentId]: m });
  function setAll(m: ShareMode) {
    const next = { ...modes };
    for (const i of settable) next[i.contentId] = m;
    modes = next;
  }
  const keep = (items: PublishItem[]) => {
    const next = { ...modes };
    for (const i of items) next[i.contentId] = "private";
    modes = next;
  };
  let listEl = $state<HTMLElement>();
  function showUploads() {
    filter = "uploads";
    query = "";
    if (listEl) listEl.scrollTop = 0;
    listEl?.scrollIntoView({ block: "nearest" });
  }

  let problemEl = $state<HTMLElement>();
  let rightsEl = $state<HTMLElement>();
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (submitting || !plan.data || !total) return;
    // Send is never greyed out for something you can fix: pressing it shows where.
    if (unchecked.length || tooBig.length || over) {
      problemEl?.scrollIntoView({ block: "nearest" });
      problemEl?.querySelector("button")?.focus();
      return;
    }
    if (uploads.length && !rights) {
      rightsError = true;
      await tick();
      rightsEl?.parentElement?.scrollIntoView({ block: "nearest" });
      rightsEl?.focus();
      return;
    }
    submitting = true;
    error = null;
    try {
      await api.publishInstance(instanceId, {
        changelog: changelog.trim() || null,
        shareModes: modes,
        confirmedUploadRights: rights,
      });
      await instances.load();
      showDone(
        people.length
          ? `Sent the update to ${people.length === 1 ? people[0].displayName : plural(people.length, "person", "people")}.`
          : "Update saved. Whoever joins gets it.",
      );
      onclose();
    } catch (err) {
      // e.g. "Someone sent a newer update. Update first."
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }

  const amber =
    "grid gap-2.5 rounded-[10px] border border-(--warn)/30 bg-(--warn)/7 p-3";
  const red =
    "grid gap-2.5 rounded-[10px] border border-(--danger)/40 bg-(--danger)/6 p-3";
</script>

<!-- How full Atlas's space will be after this update. -->
{#snippet storageBar()}
  {#if storage}
    {@const q = storage.quotaBytes}
    {@const w = (b: number) => Math.min(100, (b / q) * 100)}
    <div class="ml-6.5 flex items-center gap-2.5 text-[13px] whitespace-nowrap">
      <span
        class="relative h-1.5 min-w-20 flex-1 overflow-hidden rounded-full bg-(--line)"
        aria-hidden="true"
      >
        <span
          class="absolute inset-y-0 left-0 {over
            ? 'bg-(--danger)'
            : 'bg-(--accent)'}"
          style="width: {w(storage.usedBytes)}%"
        ></span>
        <span
          class="absolute inset-y-0 {over
            ? 'bg-(--danger)'
            : 'bg-[repeating-linear-gradient(-45deg,var(--warn)_0_4px,#c99a4b_4px_8px)]'}"
          style="left: {w(storage.usedBytes)}%; width: {Math.max(
            adds ? 1 : 0,
            w(after) - w(storage.usedBytes),
          )}%"
        ></span>
      </span>
      <span
        class="tabular-nums {over ? 'text-(--danger)' : 'text-(--text-muted)'}"
        >{over
          ? `${fileSize(after - q)} more than your ${fileSize(q)}`
          : `${fileSize(after)} of ${fileSize(q)} (${Math.round((after / q) * 100)}%)`}</span
      >
    </div>
  {/if}
{/snippet}

{#snippet itemRow(item: PublishItem, what: "new" | "updated" | "setting")}
  {@const mode = modeOf(item, modes)}
  {@const off = mode === "private"}
  {@const big =
    !!storage && toUpload(item) && item.sizeBytes > storage.maxFileBytes}
  {@const upload = item.needsUpload && !item.alreadyUploaded}
  <li
    class="flex min-h-12 items-center gap-2.5 px-2.5 py-1.25 {!off &&
    (big || item.unchecked)
      ? 'bg-(--danger)/6'
      : ''}"
  >
    <ContentIcon
      src={item.iconUrl}
      seed={item.name}
      class="size-7 rounded-[5px] {off ? 'opacity-45' : ''}"
    />
    <span class="min-w-0 flex-1">
      <b
        class="block truncate font-medium {off ? 'text-(--text-muted)' : ''}"
        title={item.name}>{item.name}</b
      >
      <small class="block truncate text-[13px] text-(--text-faint)">
        {#if what === "new"}<span class="text-(--online)">New</span
          >{:else if what === "updated"}<span class="text-(--accent)"
            >Updated</span
          >{:else}<span class="text-(--accent)">Shared differently</span>{/if},
        {KIND[item.kind]},
        {#if item.unchecked && !off}<span class="text-(--warn)"
            >couldn't be checked</span
          >{:else if upload && off}stays on your PC{:else if big}<span
            class="text-(--danger)"
            >{fileSize(item.sizeBytes)}, too big to upload</span
          >{:else if upload}<span class="text-(--warn)"
            >uploads {fileSize(item.sizeBytes)}</span
          >{:else}{item.source === "curseforge"
            ? "CurseForge"
            : item.source === "modrinth"
              ? "Modrinth"
              : "your PC"}{/if}{#if !item.enabled},
          <span class="font-medium text-(--warn)">off for you</span>{/if}
      </small>
    </span>
    <ShareModeSelect
      value={mode}
      label="How {item.name} is shared"
      onchange={(m) => setMode(item, m)}
    />
  </li>
{/snippet}

<Dialog
  title="Send an update"
  wide
  {onclose}
  onsubmit={submit}
  onescape={() =>
    // Escape closes an open menu first, not the whole dialog.
    !!document.querySelector(
      'dialog [aria-haspopup="menu"][aria-expanded="true"]',
    )}
  bodyClass="flex min-h-0 flex-1 flex-col"
>
  {#snippet head({ titleId, close })}
    <div class="flex shrink-0 items-start gap-3 pt-4.5 pr-4 pb-0.5 pl-5.5">
      <div class="min-w-0 flex-1">
        <h2
          id={titleId}
          class="font-(family-name:--font-display) text-[18px] font-semibold"
        >
          Send an update
        </h2>
        <div
          class="mt-1 flex flex-wrap items-center gap-2 text-[13px] text-(--text-muted)"
        >
          {#if people.length}
            <span class="flex">
              {#each people.slice(0, 3) as p, i (i)}
                <span
                  class="rounded-full shadow-[0_0_0_2px_var(--bg-raise)] {i
                    ? '-ml-1.5'
                    : ''}"><Head user={p} size={20} /></span
                >
              {/each}
            </span>
            To {who}
          {:else}
            Nobody has joined yet. Whoever joins gets it.
          {/if}
        </div>
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

  <div
    class="flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto px-5.5 pt-3.5 pb-4.5"
  >
    {#if plan.loading}
      <div class="grid gap-2" aria-busy="true">
        <p class="text-[13px] text-(--text-muted) tabular-nums">
          {work
            ? workLabel(work)
            : "Comparing your folder with the last update…"}
        </p>
        {#each [0, 1, 2] as n (n)}
          <div
            class="h-12 animate-pulse rounded-[10px] bg-(--bg-raise-2)"
          ></div>
        {/each}
      </div>
    {:else if plan.error}
      <p class="text-[13px] text-(--danger)">
        {plan.error}
        <button type="button" class="ml-1 underline" onclick={plan.reload}
          >Try again</button
        >
      </p>
    {:else if plan.data}
      <!-- Problems first, each about one cause, with the button that fixes it. -->
      {#if unchecked.length || tooBig.length || over}
        <div bind:this={problemEl} class="grid shrink-0 gap-2.5">
          {#if unchecked.length}
            {@const one = unchecked.length === 1}
            <section class={amber} aria-label="Files Atlas couldn't check">
              <div class="flex items-start gap-2.5">
                <Icon
                  name="warn"
                  class="mt-0.5 size-4 shrink-0 text-(--warn)"
                />
                <div class="min-w-0 flex-1 text-[13px] text-(--text-muted)">
                  <b
                    class="block text-[14px] font-semibold text-(--text-primary)"
                    >Atlas couldn't check {names(unchecked)}</b
                  >
                  Modrinth and CurseForge didn't answer, so Atlas doesn't know yet
                  whether {one ? "it has" : "they have"} to be uploaded. The update
                  waits until it can check.
                  <span class="mt-2.5 flex flex-wrap gap-2">
                    <Button size="sm" onclick={plan.reload}>Try again</Button>
                    <Button size="sm" onclick={() => keep(unchecked)}
                      >Keep {one ? "it" : "them"} to myself</Button
                    >
                  </span>
                </div>
              </div>
            </section>
          {/if}
          {#if tooBig.length && storage}
            {@const one = tooBig.length === 1}
            <section class={red} aria-label="Files too big to upload">
              <div class="flex items-start gap-2.5">
                <Icon
                  name="warn"
                  class="mt-0.5 size-4 shrink-0 text-(--danger)"
                />
                <div class="min-w-0 flex-1 text-[13px] text-(--text-muted)">
                  <b
                    class="block text-[14px] font-semibold text-(--text-primary)"
                    >{names(tooBig)} {one ? "is" : "are"} too big to upload</b
                  >
                  Uploads can be up to {fileSize(storage.maxFileBytes)} each, and
                  {one
                    ? `this one is ${fileSize(tooBig[0].sizeBytes)}`
                    : "these are bigger"}.
                  <span class="mt-2.5 flex flex-wrap gap-2">
                    <Button size="sm" onclick={() => keep(tooBig)}
                      >Keep {one ? "it" : "them"} to myself</Button
                    >
                  </span>
                </div>
              </div>
            </section>
          {/if}
          {#if over && storage}
            <section class={red} aria-label="Not enough upload space">
              <div class="flex items-start gap-2.5">
                <Icon
                  name="warn"
                  class="mt-0.5 size-4 shrink-0 text-(--danger)"
                />
                <div class="min-w-0 flex-1 text-[13px] text-(--text-muted)">
                  <b
                    class="block text-[14px] font-semibold text-(--text-primary)"
                    >Not enough upload space</b
                  >
                  This update uploads {fileSize(adds)}, and you have
                  {fileSize(
                    Math.max(0, storage.quotaBytes - storage.usedBytes),
                  )} left of your {fileSize(storage.quotaBytes)}.
                </div>
              </div>
              {@render storageBar()}
              <span class="ml-6.5 flex flex-wrap gap-2">
                <Button size="sm" onclick={showUploads}>Show the uploads</Button
                >
                <Button
                  size="sm"
                  onclick={() => openDialog({ kind: "storage" })}
                  >Free up space</Button
                >
              </span>
            </section>
          {/if}
        </div>
      {/if}

      <!-- The update itself: the note, then every change. Only its list scrolls. -->
      {@const tools = rows.length > 6 || filter !== "all"}
      <div
        class="flex min-h-0 shrink flex-col gap-2 rounded-xl border border-(--line) bg-[#141920] px-3.5 pt-3 pb-3.5"
        style="min-height: {28 +
          32 +
          46 +
          (tools ? 40 : 0) +
          (rows.length > 20 ? 40 : 0) +
          Math.min(148, Math.max(1, shown.length) * 49 + 2)}px"
      >
        <div class="flex shrink-0 items-center gap-2.5">
          <ContentIcon
            src={instance?.icon ?? null}
            seed={instance ? iconSeed(instance) : instanceId}
            class="size-6 rounded-[5px]"
          />
          <b class="min-w-0 flex-1 truncate font-semibold">{instance?.name}</b>
          <span
            class="grid h-5.5 place-items-center rounded-full bg-(--accent)/12 px-2.25 text-[13px] font-semibold text-(--accent) tabular-nums"
            >v{version}</span
          >
        </div>
        <textarea
          bind:value={changelog}
          maxlength="2000"
          rows="1"
          placeholder="Say what's new (optional)"
          aria-label="Note for friends"
          class="[field-sizing:content] max-h-25 min-h-9.5 w-full shrink-0 resize-none rounded-lg border border-(--line)/60 bg-(--bg) px-2.5 py-2 leading-snug font-medium outline-none placeholder:font-normal placeholder:text-(--text-faint) hover:border-(--line) focus:border-(--accent)"
        ></textarea>
        {#if tools}
          <div class="flex shrink-0 flex-wrap items-center gap-2">
            <div class="flex flex-wrap gap-1" role="group" aria-label="Show">
              {#each FILTERS.filter(([f]) => f === "all" || counts[f]) as [f, label] (f)}
                <button
                  type="button"
                  aria-pressed={filter === f}
                  onclick={() => {
                    filter = f;
                    if (listEl) listEl.scrollTop = 0;
                  }}
                  class="h-7 rounded-full px-2.75 text-[13px] font-medium {filter ===
                  f
                    ? 'bg-(--accent)/15 text-(--accent)'
                    : 'text-(--text-muted) hover:bg-(--bg-hover)'}"
                  >{label}<span class="ml-1 font-normal tabular-nums opacity-70"
                    >{counts[f]}</span
                  ></button
                >
              {/each}
            </div>
            {#if settable.length > 1}
              <span class="ml-auto">
                <Menu
                  label="Set every file shown"
                  items={[
                    ["required", "Required"],
                    ["optional", "Optional"],
                    ["private", "Only me"],
                  ].map(([m, label]) => ({
                    label,
                    onselect: () => setAll(m as ShareMode),
                  }))}
                  triggerClass="inline-flex h-7.5 items-center gap-1 rounded-md border border-(--line) px-2.5 text-[13px] font-medium text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary) aria-expanded:bg-(--bg-hover)"
                >
                  {#snippet trigger()}
                    Set all shown
                    <Icon name="chev" class="size-3 rotate-90" />
                  {/snippet}
                </Menu>
              </span>
            {/if}
          </div>
          {#if rows.length > 20}
            <SearchBox
              bind:value={query}
              placeholder="Search {plural(rows.length, 'change')}"
              class="shrink-0"
            />
          {/if}
        {/if}
        <ul
          bind:this={listEl}
          class="min-h-0 flex-1 overflow-y-auto rounded-[10px] border border-(--line)/60 bg-(--bg-raise) [&>*+*]:border-t [&>*+*]:border-(--line)/60"
          aria-label="Changes in version {version}"
        >
          {#each shown as r, i (i)}
            {#if r.kind === "item"}
              {@render itemRow(r.item, r.what)}
            {:else if r.kind === "removed"}
              <li class="flex min-h-12 items-center gap-2.5 px-2.5 py-1.25">
                <ContentIcon
                  src={r.item.iconUrl}
                  seed={r.item.name}
                  class="size-7 rounded-[5px]"
                />
                <span class="min-w-0 flex-1">
                  <b class="block truncate font-medium">{r.item.name}</b>
                  <small class="block text-[13px] text-(--text-faint)"
                    ><span class="text-(--danger)">Removed</span>, {KIND[
                      r.item.kind
                    ]}</small
                  >
                </span>
              </li>
            {:else if r.kind === "configs" && plan.data}
              {@const files = plan.data.filesChanged}
              <li class="flex min-h-12 items-center gap-2.5 px-2.5 py-1.25">
                <span
                  class="grid size-7 shrink-0 place-items-center rounded-[5px] bg-(--bg-raise-2) text-(--text-muted)"
                  ><Icon name="file" class="size-4" /></span
                >
                <span class="min-w-0 flex-1">
                  <b class="block font-medium"
                    >{plural(files.length, "file")} changed</b
                  >
                  <small
                    class="block truncate text-[13px] text-(--text-faint)"
                    title={files.join(", ")}
                    >{nameList(
                      files.map((f) => f.split("/").pop() ?? f),
                    )}</small
                  >
                </span>
              </li>
            {:else if r.kind === "versions" && plan.data?.versions}
              {@const v = plan.data.versions}
              <li class="flex min-h-12 items-center gap-2.5 px-2.5 py-1.25">
                <ContentIcon
                  src={null}
                  seed="minecraft"
                  class="size-7 rounded-[5px]"
                />
                <span class="min-w-0 flex-1">
                  {#if v.minecraftVersion}<b class="block font-medium"
                      >Minecraft {v.minecraftVersion.from} → {v.minecraftVersion
                        .to}</b
                    >{/if}
                  {#if v.loaderVersion}<small
                      class="block text-[13px] text-(--text-faint)"
                      >{versionLabel(instance?.loader ?? null, null)}
                      {v.loaderVersion.from} → {v.loaderVersion.to}</small
                    >{/if}
                </span>
              </li>
            {/if}
          {:else}
            <li class="px-4 py-6 text-center text-[13px] text-(--text-muted)">
              {#if query}No changes match “{query}”.{:else}Nothing to send yet.
                Change files in your launcher, or how they're shared in the
                Content tab.{/if}
            </li>
          {/each}
        </ul>
      </div>

      {#if uploads.length && !over}
        {@const one = uploads.length === 1}
        <section class="{amber} shrink-0" aria-label="Uploads to Atlas">
          <div class="flex items-start gap-2.5 text-[13px] text-(--text-muted)">
            <Icon name="upload" class="mt-0.5 size-4 shrink-0 text-(--warn)" />
            <span
              ><b class="font-semibold text-(--text-primary)"
                >{nameList(uploads.map((i) => i.name))}</b
              >
              {one ? "uploads" : "upload"} to Atlas ({fileSize(adds)}), since {one
                ? "it isn't"
                : "they aren't"} on Modrinth or CurseForge.</span
            >
          </div>
          {@render storageBar()}
          <div>
            <label
              class="tap flex cursor-pointer items-start gap-2.5 text-[13px]"
            >
              <input
                bind:this={rightsEl}
                type="checkbox"
                bind:checked={rights}
                onchange={() => rights && (rightsError = false)}
                aria-invalid={rightsError && !rights}
                aria-describedby={rightsError && !rights
                  ? "rights-error"
                  : undefined}
                class="mt-0.5 size-4 shrink-0 accent-(--accent) {rightsError &&
                !rights
                  ? 'outline-2 outline-offset-2 outline-(--danger)'
                  : ''}"
              />
              <span
                >I made {one ? "it" : "them"}, or have permission to share {one
                  ? "it"
                  : "them"}</span
              >
            </label>
            {#if rightsError && !rights}
              <p
                id="rights-error"
                role="alert"
                class="mt-1 ml-6.5 flex items-start gap-1.5 text-[13px] text-(--danger)"
              >
                <Icon name="warn" class="mt-0.75 size-3.5 shrink-0" />Tick this
                first. Atlas only uploads files you're allowed to share.
              </p>
            {/if}
          </div>
        </section>
      {/if}
    {/if}
  </div>

  {#snippet footer()}
    {#if submitting || error}
      <span
        class="mr-auto text-[13px] tabular-nums {error
          ? 'text-(--danger)'
          : 'text-(--text-muted)'}"
        role={error ? "alert" : undefined}
        >{error ?? (work ? workLabel(work) : "Sending…")}</span
      >
    {/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={submitting || !total}>
      {submitting ? "Sending…" : "Send update"}
    </Button>
  {/snippet}
</Dialog>
