<script lang="ts">
  import '../app.css';
  import MathSyncOverlay from '$lib/components/MathSyncOverlay.svelte';
  let { children } = $props();
</script>

{@render children()}
<MathSyncOverlay />

