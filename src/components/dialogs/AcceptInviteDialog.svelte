<script lang="ts">
  import { api } from "$lib/api";
  import { fileSize, plural, versionLabel } from "$lib/format";
  import { openDialog } from "$lib/dialogs.svelte";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id, InstanceSummary, Launcher } from "$lib/types";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import OptionalPicker from "../OptionalPicker.svelte";

  let {
    sharedInstanceId,
    onclose,
  }: { sharedInstanceId: Id; onclose: () => void } = $props();

  const invite = $derived(social.shared.find((s) => s.id === sharedInstanceId));
  const launchers = resource(() => api.listLaunchers());
  // Launchers that can make a new instance on the invite's loader.
  const creatable = $derived(
    launchers.data?.filter(
      (l) => !!invite && l.creatableLoaders.includes(invite.loader),
    ) ?? [],
  );

  // Instances already tracked here that aren't shared: the files can go into one of those.
  const spare = $derived(
    instances.list.filter((i) => !i.sync && i.status !== "missing"),
  );

  // Where the files go: a new instance in a launcher, or an instance you already have.
  type Target = { kind: "launcher" | "instance"; id: Id };

  // A *writable* $derived: it starts as "the first launcher that can receive
  // instances" (or the first spare instance), and fills in by itself once the lists
  // load, but assigning to it overrides that — which is what clicking a radio does.
  let target = $derived<Target | null>(
    creatable[0]
      ? { kind: "launcher", id: creatable[0].id }
      : spare[0]
        ? { kind: "instance", id: spare[0].id }
        : null,
  );
  const picked = (kind: Target["kind"], id: Id) =>
    target?.kind === kind && target.id === id;

  // Optional extras the user ticks (none to begin with).
  let optional = $state<string[]>([]);

  let submitting = $state(false);
  let error = $state<string | null>(null);

  const canSubmit = $derived(!submitting && !!invite && !!target);
  const pickedLauncher = $derived(
    target?.kind === "launcher"
      ? launchers.data?.find((l) => l.id === target?.id)
      : undefined,
  );
  const pickedInstance = $derived(
    target?.kind === "instance"
      ? spare.find((i) => i.id === target?.id)
      : undefined,
  );

  /** The Minecraft Launcher calls them profiles. */
  const newWhat = (l: Launcher) =>
    l.kind === "official" ? "profile" : "instance";

  /** One line on what happens when this launcher is picked. */
  function newHint(l: Launcher) {
    const items = invite ? plural(invite.contentCount, "item") : "the files";
    switch (l.kind) {
      case "modrinth":
        return `The Modrinth App asks you to confirm, then Atlas adds the ${items}`;
      case "official":
        return `Close the launcher first, then Atlas adds the ${items}`;
      case "curseforge":
        return `Close CurseForge first, then Atlas adds the ${items}`;
      default:
        return `Atlas creates it and adds the ${items}`;
    }
  }

  // The button says what it will do.
  const action = $derived(
    pickedLauncher
      ? `Create in ${pickedLauncher.name}`
      : pickedInstance
        ? `Add to ${pickedInstance.name}`
        : "Add",
  );
  // Nowhere to put it yet: the button tracks an instance instead.
  const nowhere = $derived(
    !!invite && !launchers.loading && !creatable.length && !spare.length,
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

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit || !target) return;
    submitting = true;
    error = null;
    try {
      const created =
        target.kind === "launcher"
          ? await api.acceptInvite(sharedInstanceId, target.id, optional)
          : await api.acceptInviteInto(sharedInstanceId, target.id, optional);
      await Promise.all([instances.load(), social.load()]);
      onclose();
      // Open the new instance: its sync bar shows the files arriving.
      go({ page: "instance", id: created.id });
    } catch (err) {
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }
</script>

<!-- One choice: the whole card is the click target; the radio stays for keyboards and screen readers. -->
{#snippet choice(
  kind: Target["kind"],
  id: Id,
  title: string,
  hint: string,
  instance: InstanceSummary | null,
  warn = false,
)}
  {@const on = picked(kind, id)}
  <label
    class="flex cursor-pointer items-center gap-3 rounded-lg border px-3 py-2.5 transition-colors
      has-focus-visible:outline-2 has-focus-visible:outline-(--accent)
      {on
      ? 'border-(--accent) bg-(--accent)/8'
      : 'border-(--line-2) hover:bg-(--bg-hover)'}"
  >
    <input
      type="radio"
      name="target"
      value={id}
      checked={on}
      onchange={() => (target = { kind, id })}
      class="sr-only"
    />
    {#if instance}
      <ContentIcon
        src={instance.icon}
        seed={instance.id}
        class="size-8 shrink-0 rounded-md"
      />
    {:else}
      <span
        class="grid size-8 shrink-0 place-items-center rounded-md bg-(--bg-raise-2) text-(--accent)"
      >
        <Icon name="plus" class="size-4" />
      </span>
    {/if}
    <span class="min-w-0 flex-1">
      <b class="block truncate font-medium">{title}</b>
      <small
        class="block text-[11.5px] {warn
          ? 'text-(--warn)'
          : 'text-(--text-faint)'}">{hint}</small
      >
    </span>
    <span
      class="grid size-5 shrink-0 place-items-center rounded-full border {on
        ? 'border-(--accent) bg-(--accent) text-(--accent-ink)'
        : 'border-(--line-2)'}"
    >
      {#if on}<Icon name="check" class="size-3" />{/if}
    </span>
  </label>
{/snippet}

<Dialog
  title={invite ? `Add ${invite.name}` : "Shared instance"}
  description={invite
    ? `${versionLabel(invite.loader, invite.minecraftVersion)} · ${plural(invite.contentCount, "item")} · ${fileSize(invite.sizeBytes)}`
    : undefined}
  {onclose}
  onsubmit={submit}
>
  {#snippet header()}
    <ContentIcon
      src={invite?.icon ?? null}
      seed={sharedInstanceId}
      class="size-10 rounded-md"
    />
  {/snippet}

  {#if !invite}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      This shared instance isn't available anymore.
    </p>
  {:else if submitting}
    <!-- While it works: say what's happening and what, if anything, to do. -->
    <div class="flex items-start gap-3.5 rounded-lg bg-(--bg-raise) px-4 py-4">
      <Icon
        name="refresh"
        class="mt-0.5 size-5 shrink-0 animate-spin text-(--accent)"
      />
      <div class="text-[12.5px]">
        {#if pickedLauncher?.kind === "modrinth"}
          <b class="block text-[13.5px] font-semibold"
            >Confirm in the Modrinth App</b
          >
          <p class="mt-1 text-(--text-muted)">
            It has opened and is asking whether to add “{invite.name}”. Confirm
            there, and Atlas adds the {plural(invite.contentCount, "item")} by itself.
          </p>
        {:else if pickedLauncher}
          <b class="block text-[13.5px] font-semibold"
            >Creating the {newWhat(pickedLauncher)} in {pickedLauncher.name}…</b
          >
          <p class="mt-1 text-(--text-muted)">
            Then Atlas adds the {plural(invite.contentCount, "item")}.
          </p>
        {:else}
          <b class="block text-[13.5px] font-semibold"
            >Linking {pickedInstance?.name ?? "the instance"}…</b
          >
          <p class="mt-1 text-(--text-muted)">
            Then Atlas adds the {plural(invite.contentCount, "item")}.
          </p>
        {/if}
      </div>
    </div>
  {:else}
    {#if launchers.loading}
      <p class="text-[12.5px] text-(--text-muted)">Looking for launchers…</p>
    {:else if launchers.error}
      <p class="text-[12.5px] text-(--danger)">{launchers.error}</p>
    {:else if nowhere}
      <p class="rounded-lg bg-(--bg-raise) px-3.5 py-3 text-[12.5px]">
        <b class="mb-0.5 block font-medium">Nowhere to add it yet</b>
        <span class="text-(--text-muted)">
          Make an empty {versionLabel(invite.loader, invite.minecraftVersion)}
          instance in your launcher, then track it here.
        </span>
      </p>
    {:else}
      <h3 class="mt-1 mb-2 text-[13px] font-semibold">Where should it go?</h3>

      {#if creatable.length}
        <p class="mb-1.5 text-[11.5px] font-medium text-(--text-faint)">New</p>
        <div class="flex flex-col gap-1.5">
          {#each creatable as l (l.id)}
            {@render choice(
              "launcher",
              l.id,
              `New ${newWhat(l)} in ${l.name}`,
              newHint(l),
              null,
            )}
          {/each}
        </div>
      {/if}

      {#if spare.length}
        <p
          class="mb-1.5 text-[11.5px] font-medium text-(--text-faint) {creatable.length
            ? 'mt-4'
            : ''}"
        >
          An instance you already have
        </p>
        <div class="flex flex-col gap-1.5">
          {#each spare as i (i.id)}
            {@const note = mismatch(i)}
            {@render choice(
              "instance",
              i.id,
              i.name,
              note ?? `${i.launcher.name} · your own files there stay`,
              i,
              !!note,
            )}
          {/each}
        </div>
      {/if}
    {/if}

    <OptionalPicker items={invite.optionalItems} bind:selected={optional} />
  {/if}

  {#snippet footer()}
    {#if error}<span class="mr-auto text-xs text-(--danger)" role="alert"
        >{error}</span
      >{/if}
    <!-- Closing doesn't stop it: the work carries on and the instance shows up when done. -->
    <Button variant="ghost" onclick={onclose}
      >{submitting ? "Close" : "Cancel"}</Button
    >
    {#if nowhere}
      <Button variant="primary" onclick={() => openDialog({ kind: "track" })}>
        Track an instance
      </Button>
    {:else}
      <Button type="submit" variant="primary" disabled={!canSubmit}>
        {submitting ? "Working…" : action}
      </Button>
    {/if}
  {/snippet}
</Dialog>
