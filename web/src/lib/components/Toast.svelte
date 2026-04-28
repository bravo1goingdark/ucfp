<script lang="ts">
  import { toasts, dismiss } from '$lib/stores/toasts.svelte';
</script>

<div class="toast-region" role="region" aria-label="Notifications" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}">
      <span class="msg">{t.message}</span>
      <button
        type="button"
        class="close"
        aria-label="Dismiss notification"
        onclick={() => dismiss(t.id)}
      >×</button>
    </div>
  {/each}
</div>
