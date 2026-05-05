<!--
  Fullbleed viewer layout. The trailing `@` on the file name resets layout
  inheritance to the root, so we render WITHOUT the dashboard sidebar and
  top bar. The viewer pages take over the entire viewport and let users
  inspect a single visualization with maximum screen real estate.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';

  let { children } = $props();

  function back() {
    if (typeof window !== 'undefined' && window.history.length > 1) {
      window.history.back();
    } else {
      goto('/dashboard/playground');
    }
  }
</script>

<div class="viewer-shell">
  <header class="viewer-bar">
    <button type="button" class="viewer-back" onclick={back} aria-label="Back">← back</button>
    <span class="viewer-title">UCFP · viewer · {$page.url.pathname.split('/').filter(Boolean).slice(-2).join(' / ')}</span>
    <a class="viewer-exit" href="/dashboard/playground" aria-label="Exit to playground">×</a>
  </header>
  <main class="viewer-main">
    {@render children()}
  </main>
</div>

<style>
  .viewer-shell {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--ink);
    z-index: 1;
  }
  .viewer-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    color: var(--muted);
    background: var(--bg-2);
  }
  .viewer-back {
    background: transparent;
    border: 1px solid var(--line-strong);
    color: var(--ink-2);
    font-family: var(--mono);
    font-size: 11px;
    padding: 5px 10px;
    cursor: pointer;
    letter-spacing: 0.06em;
  }
  .viewer-back:hover {
    background: var(--ink);
    color: var(--bg);
    border-color: var(--ink);
  }
  .viewer-title {
    flex: 1;
    text-align: center;
    text-transform: uppercase;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .viewer-exit {
    text-decoration: none;
    color: var(--ink-2);
    font-size: 18px;
    line-height: 1;
    padding: 2px 10px;
    border: 1px solid var(--line-strong);
  }
  .viewer-exit:hover {
    background: var(--ink);
    color: var(--bg);
  }
  .viewer-main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
