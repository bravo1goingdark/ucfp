<script lang="ts">
  let { children } = $props();
</script>

<div class="auth-shell">
  <a class="brand" href="/">
    <span class="glyph" aria-hidden="true">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <circle cx="6" cy="6" r="1.2" fill="currentColor" />
        <circle cx="6" cy="6" r="3" stroke="currentColor" stroke-width="0.8" fill="none" />
        <path d="M1 6a5 5 0 0 1 10 0" stroke="currentColor" stroke-width="0.8" fill="none" />
        <path d="M2.5 9a3.5 3.5 0 0 1 7 0" stroke="currentColor" stroke-width="0.8" fill="none" />
      </svg>
    </span>
    <b>UCFP</b>
  </a>
  <div class="auth-card">
    {@render children()}
  </div>
</div>

<style>
  .auth-shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 32px;
    padding: 40px 20px;
  }
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    text-decoration: none;
    color: var(--ink);
    font-family: var(--mono);
    font-size: 13px;
    letter-spacing: 0.04em;
  }
  .brand .glyph {
    width: 22px;
    height: 22px;
    border: 1px solid var(--ink);
    display: grid;
    place-items: center;
  }
  .brand b {
    font-weight: 600;
    letter-spacing: 0.08em;
  }
  .auth-card {
    width: 100%;
    max-width: 420px;
    background: #FBF9F3;
    border: 1px solid var(--line-strong);
    box-shadow: var(--paper-shadow);
    padding: 36px 32px;
  }
  :global([data-theme='ink']) .auth-card { background: var(--bg-2); }
</style>
