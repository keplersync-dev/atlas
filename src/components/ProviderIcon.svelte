<script lang="ts">
  import BlockArt from "./BlockArt.svelte";

  let { class: className = "size-6" }: { class?: string } = $props();
</script>

<!-- A grass block: Minecraft is the one way to sign in. -->
<BlockArt seed="minecraft" kind="grass" class="rounded-[4px] {className}" />
