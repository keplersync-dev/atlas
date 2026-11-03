<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    value = $bindable(),
    placeholder,
    label,
    raised = false,
    class: className = "",
  }: {
    value: string;
    placeholder: string;
    /** For screen readers when the placeholder says more, like a count. */
    label?: string;
    /** On the page itself rather than inside a card or dialog. */
    raised?: boolean;
    class?: string;
  } = $props();
</script>

<label class="relative block {className}">
  <span
    class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
  >
    <Icon name="search" class="size-3.5" />
  </span>
  <input
    bind:value
    {placeholder}
    aria-label={label ?? placeholder}
    class="h-8 w-full rounded-md border border-(--line-2) pr-2.5 pl-8 text-[13px] outline-none placeholder:text-(--text-faint) focus:border-(--accent) {raised
      ? 'bg-(--bg-raise)'
      : 'bg-(--bg)'}"
  />
</label>
