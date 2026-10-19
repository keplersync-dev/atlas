<script lang="ts" module>
  import type { Id, PublishPlan } from "$lib/types";

  /** Bytes the selected files will add to the user's storage. Exported so dialogs can check the quota too. */
  export function uploadBytes(
    files: PublishPlan["unknownFiles"],
    selected: Id[],
  ) {
    return files
      .filter((f) => selected.includes(f.contentId) && !f.alreadyUploaded)
      .reduce((n, f) => n + f.sizeBytes, 0);
  }
</script>

<script lang="ts">
  import { fileSize, plural } from "$lib/format";

  // "N files aren't on Modrinth or CurseForge": choose which to upload to Atlas.
  // Both `selected` and `rights` are $bindable, so the parent writes
  //   <UploadPicker bind:selected={uploadIds} bind:rights={confirmed} … />
  // and always has the current values.
  let {
    files,
    storage,
    selected = $bindable([]),
    rights = $bindable(false),
  }: {
    files: PublishPlan["unknownFiles"];
    storage: PublishPlan["storage"];
    selected: Id[];
    rights: boolean;
  } = $props();

  // Files already on the server cost nothing; only new uploads count against the quota.
  const adding = $derived(uploadBytes(files, selected));
  const usedAfter = $derived(storage.usedBytes + adding);
  const overQuota = $derived(usedAfter > storage.quotaBytes);
</script>

<h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
  {plural(files.length, "file")} not on Modrinth or CurseForge
</h3>
<div class="rounded-lg bg-(--bg-raise) px-3.5 py-3">
  <p class="mb-1.5 text-[12.5px] text-(--text-muted)">
    Friends can't download {files.length === 1 ? "this" : "these"} anywhere else.
    Upload to Atlas, or leave
    {files.length === 1 ? "it" : "them"} on your PC only.
  </p>

  {#each files as f (f.contentId)}
    {@const tooBig = f.sizeBytes > storage.maxFileBytes}
    <label
      class="-mx-2 flex items-center gap-3 rounded-md px-2 py-1.5 {tooBig
        ? 'opacity-50'
        : 'cursor-pointer hover:bg-(--bg-hover)'}"
    >
      <!-- bind:group: every checkbox bound to the same array. Checked = its value is in the array. -->
      <input
        type="checkbox"
        bind:group={selected}
        value={f.contentId}
        disabled={tooBig}
        class="size-3.75 accent-(--accent)"
      />
      <span class="min-w-0 flex-1">
        <b class="block truncate font-medium">{f.name}</b>
        <small
          class="block truncate font-(family-name:--font-mono) text-[11.5px] text-(--text-faint)"
        >
          {tooBig
            ? `Larger than the ${fileSize(storage.maxFileBytes)} limit`
            : f.path}
        </small>
      </span>
      <span
        class="font-(family-name:--font-mono) text-[11.5px] text-(--text-faint)"
      >
        {f.alreadyUploaded ? "uploaded" : fileSize(f.sizeBytes)}
      </span>
    </label>
  {/each}

  <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-(--line-2)">
    <div
      class="h-full transition-[width] {overQuota
        ? 'bg-(--danger)'
        : 'bg-(--accent)'}"
      style="width: {Math.min(100, (usedAfter / storage.quotaBytes) * 100)}%"
    ></div>
  </div>
  <div
    class="mt-1.5 flex justify-between text-[11.5px] {overQuota
      ? 'text-(--danger)'
      : 'text-(--text-faint)'}"
  >
    <span>
      {fileSize(usedAfter)} of {fileSize(storage.quotaBytes)} used{adding
        ? " after upload"
        : ""}
    </span>
    {#if overQuota}<span>Not enough space — untick some files</span>{/if}
  </div>

  {#if selected.length}
    <label class="mt-2.5 flex cursor-pointer items-start gap-2.5 text-[12.5px]">
      <!-- bind:checked: a single checkbox bound to a boolean -->
      <input
        type="checkbox"
        bind:checked={rights}
        class="mt-0.5 accent-(--accent)"
      />
      <span>I made these files or have permission to share them.</span>
    </label>
  {/if}
</div>
