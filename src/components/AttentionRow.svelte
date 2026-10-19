<script lang="ts">
  import type { Snippet } from "svelte";

  // A row in Home's "Needs you" list. Same layout every time; the parent fills
  // in the picture (`art`), the sentence (`children`) and the buttons (`actions`)
  // as snippets — pieces of markup passed in like props.
  let {
    kicker,
    tone = "accent",
    detail,
    art,
    actions,
    children,
  }: {
    /** Small label above the sentence: "Update", "Invite"… */
    kicker: string;
    tone?: "accent" | "warn";
    /** Grey line under the sentence. */
    detail: string;
    art: Snippet;
    actions: Snippet;
    children: Snippet;
  } = $props();
</script>

<div class="flex items-center gap-3.5 px-4 py-3.5">
  {@render art()}
  <div class="min-w-0 flex-1">
    <span
      class="text-[11.5px] font-semibold {tone === 'warn'
        ? 'text-(--warn)'
        : 'text-(--accent)'}">{kicker}</span
    >
    <div class="[&_b]:font-semibold">{@render children()}</div>
    <small class="mt-0.5 block truncate text-xs text-(--text-muted)"
      >{detail}</small
    >
  </div>
  <div class="flex shrink-0 gap-2">{@render actions()}</div>
</div>
