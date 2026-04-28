<script lang="ts">
  import '../app.css';
  import Tweaks from '$components/Tweaks.svelte';

  let { children } = $props();
</script>

{@render children()}

<Tweaks />
