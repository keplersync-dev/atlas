<script lang="ts">
  // Renders an icon from /static as a mask, so it takes the current text color.
  let { name, class: className = "size-4" }: { name: string; class?: string } =
    $props();
</script>

<span
  aria-hidden="true"
  class="inline-block shrink-0 bg-current {className}"
  style="mask: url(/{name}.svg) no-repeat center / contain; -webkit-mask: url(/{name}.svg) no-repeat center / contain;"
></span>
