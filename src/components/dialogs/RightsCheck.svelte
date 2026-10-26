<script lang="ts">
  // The upload-rights confirmation, in a dialog's footer next to its buttons so it's
  // always in view. `note` goes under it: progress, or why sharing failed.
  let {
    checked = $bindable(false),
    note = null,
    tone = "muted",
  }: {
    checked: boolean;
    note?: string | null;
    tone?: "muted" | "danger";
  } = $props();
</script>

<div class="mr-auto flex min-w-0 flex-col gap-1">
  <label class="flex cursor-pointer items-center gap-2 text-[12.5px]">
    <input type="checkbox" bind:checked class="accent-(--accent)" />
    <span>I made these files or have permission to share them</span>
  </label>
  {#if note}
    <span
      class="text-xs tabular-nums {tone === 'danger'
        ? 'text-(--danger)'
        : 'text-(--text-muted)'}"
      role={tone === "danger" ? "alert" : undefined}>{note}</span
    >
  {/if}
</div>
