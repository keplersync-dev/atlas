<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  let {
    class: className = "",
    selected = false,
    children,
    ...props
  }: HTMLButtonAttributes & {
    selected?: boolean;
  } = $props();
</script>

<button
  {...props}
  class="
    relative flex items-center gap-1.5
    px-2
    text-[14px]
    leading-5
    font-normal whitespace-nowrap text-(--text-primary)
    transition-colors
    duration-200 select-none
    hover:bg-(--bg-hover)
    [&_img]:size-4.5
    {className}
  "
>
  {@render children?.()}

  <span
    class="
    absolute right-2 bottom-0 left-2
    h-px bg-(--accent)
    transition-[opacity,transform] duration-200
    {selected ? 'scale-x-100 opacity-100' : 'scale-x-0 opacity-0'}
  "
  ></span>
</button>
