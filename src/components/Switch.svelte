<script lang="ts">
  // A "controlled" switch: it shows `checked` and reports clicks through
  // `onchange`, but never flips itself. The parent decides what really happens
  // (call the API, refuse while the game is running…) and passes the new value back.
  let {
    checked,
    onchange,
    disabled = false,
    label,
  }: {
    checked: boolean;
    onchange: (next: boolean) => void;
    disabled?: boolean;
    /** For screen readers, e.g. "Disable Sodium". */
    label: string;
  } = $props();
</script>

<!-- 36 × 24 to click around a 30 × 18 switch; the margin keeps its place in the row. -->
<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange(!checked)}
  class="group -m-0.75 grid h-6 w-9 shrink-0 place-items-center focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40"
>
  <span
    class="relative h-4.5 w-7.5 rounded-full transition-colors group-focus-visible:outline-2 group-focus-visible:outline-offset-2 group-focus-visible:outline-(--accent)
      {checked ? 'bg-(--accent)' : 'bg-(--line-2)'}"
  >
    <span
      class="absolute top-0.75 left-0.75 size-3 rounded-full transition-transform
        {checked ? 'translate-x-3 bg-(--accent-ink)' : 'bg-(--text-primary)'}"
    ></span>
  </span>
</button>
