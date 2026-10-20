<script lang="ts" module>
  import type { ShareMode } from "$lib/types";

  export const SHARE_MODES: { id: ShareMode; label: string; hint: string }[] = [
    {
      id: "required",
      label: "Required",
      hint: "Everyone who has this instance gets it",
    },
    {
      id: "optional",
      label: "Optional",
      hint: "Friends see it and choose whether to download it",
    },
    { id: "private", label: "Only me", hint: "Stays on your PC, never shared" },
  ];
</script>

<script lang="ts">
  // Three-way control for how one file is shared. Controlled: shows `value`,
  // reports clicks through `onchange`, the parent decides what happens.
  let {
    value,
    onchange,
    disabled = false,
    label,
  }: {
    value: ShareMode;
    onchange: (next: ShareMode) => void;
    disabled?: boolean;
    /** For screen readers, e.g. "How Sodium is shared". */
    label: string;
  } = $props();

  const ACTIVE: Record<ShareMode, string> = {
    required: "bg-(--bg-raise-2) text-(--text-primary)",
    optional: "bg-(--accent)/15 text-(--accent)",
    private: "bg-(--bg-raise-2) text-(--text-muted)",
  };
</script>

<div
  role="radiogroup"
  aria-label={label}
  class="inline-flex shrink-0 gap-0.5 rounded-md bg-(--bg) p-0.5 {disabled
    ? 'opacity-50'
    : ''}"
>
  {#each SHARE_MODES as m (m.id)}
    <button
      type="button"
      role="radio"
      aria-checked={value === m.id}
      title={m.hint}
      {disabled}
      onclick={() => value !== m.id && onchange(m.id)}
      class="rounded px-2 py-0.5 text-[11.5px] font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed
        {value === m.id
        ? ACTIVE[m.id]
        : 'text-(--text-faint) hover:text-(--text-primary)'}"
    >
      {m.label}
    </button>
  {/each}
</div>
