<script lang="ts">
  import { page } from '$app/stores';
</script>

<svelte:head>
  <title>{$page.status} — UCFP</title>
</svelte:head>

<div class="err-shell">
  <div class="err-card">
    <div class="status">{$page.status}</div>
    <h1>{$page.error?.message ?? 'Something went wrong.'}</h1>
    {#if $page.error && 'id' in $page.error && (($page.error as { id?: string }).id)}
      <p class="ref">
        Reference: <code>{($page.error as { id: string }).id}</code>
      </p>
    {/if}
    <div class="actions">
      <a class="btn" href="/">Back to home →</a>
      <a class="btn alt" href="/docs/getting-started">Read the docs →</a>
    </div>
  </div>
</div>

<style>
  .err-shell {
    min-height: 70vh;
    display: grid;
    place-items: center;
    padding: 64px 24px;
  }
  .err-card {
    max-width: 520px;
    text-align: center;
    border: 1px solid var(--line-strong);
    background: #FBF9F3;
    box-shadow: var(--paper-shadow);
    padding: 48px 36px;
  }
  :global([data-theme='ink']) .err-card { background: var(--bg-2); }
  .status {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.18em;
    color: var(--muted);
    margin-bottom: 18px;
  }
  h1 {
    font-family: var(--sans);
    font-weight: 400;
    font-size: 32px;
    letter-spacing: -0.02em;
    line-height: 1.1;
    margin: 0 0 18px;
  }
  .ref {
    color: var(--muted);
    font-family: var(--mono);
    font-size: 12px;
    margin: 0 0 28px;
  }
  .ref code {
    color: var(--ink);
  }
  .actions {
    display: flex;
    justify-content: center;
    gap: 12px;
    flex-wrap: wrap;
  }
</style>
