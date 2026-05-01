<script lang="ts">
  // Hamburger toggle for the mobile nav. Closed by default; the hamburger
  // button is hidden on >900 px, where the regular nav links render inline.
  let mobileOpen = $state(false);

  function close() { mobileOpen = false; }
</script>

<nav class="top">
  <div class="brand">
    <span class="glyph" aria-hidden="true">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <circle cx="6" cy="6" r="1.2" fill="currentColor" />
        <circle cx="6" cy="6" r="3" stroke="currentColor" stroke-width="0.8" fill="none" />
        <path d="M1 6a5 5 0 0 1 10 0" stroke="currentColor" stroke-width="0.8" fill="none" />
        <path d="M2.5 9a3.5 3.5 0 0 1 7 0" stroke="currentColor" stroke-width="0.8" fill="none" />
      </svg>
    </span>
    <b>UCFP</b>
    <span class="dot">/</span>
    <span style="color: var(--muted)">v0.4.1</span>
  </div>

  <div class="nav-links">
    <a href="#demo">Demo</a>
    <a href="/docs/getting-started">Docs</a>
    <a href="#features">Capabilities</a>
    <a href="#pricing">Pricing</a>
    <a href="#faq">FAQ</a>
  </div>

  <div class="nav-auth">
    <a class="nav-login" href="/login">Log in</a>
    <a class="nav-cta" href="/signup">Get started →</a>
  </div>

  <button
    type="button"
    class="nav-burger"
    aria-label={mobileOpen ? 'Close menu' : 'Open menu'}
    aria-expanded={mobileOpen}
    aria-controls="mobile-nav-panel"
    onclick={() => (mobileOpen = !mobileOpen)}
  >
    {#if mobileOpen}
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
        <path d="M5 5l10 10M15 5L5 15" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    {:else}
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
        <path d="M3 6h14M3 10h14M3 14h14" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    {/if}
  </button>
</nav>

{#if mobileOpen}
  <button type="button" class="mobile-nav-backdrop" aria-label="Close menu" onclick={close}></button>
  <aside id="mobile-nav-panel" class="mobile-nav" aria-label="Mobile navigation">
    <a href="#demo" onclick={close}>Demo</a>
    <a href="/docs/getting-started" onclick={close}>Docs</a>
    <a href="#features" onclick={close}>Capabilities</a>
    <a href="#pricing" onclick={close}>Pricing</a>
    <a href="#faq" onclick={close}>FAQ</a>
    <hr />
    <a href="/login" onclick={close}>Log in</a>
    <a class="cta" href="/signup" onclick={close}>Get started →</a>
  </aside>
{/if}

<style>
  /* Hamburger is hidden on the wide layout — `.nav-links` carries them inline. */
  .nav-burger {
    display: none;
    appearance: none;
    border: 1px solid var(--ink, #111);
    background: transparent;
    color: inherit;
    width: 36px;
    height: 36px;
    border-radius: 0.4rem;
    cursor: pointer;
    align-items: center;
    justify-content: center;
  }
  .nav-burger:hover { background: var(--bg-2, rgba(0,0,0,0.04)); }
  .nav-burger:focus-visible { outline: 2px solid var(--accent, #6ad); outline-offset: 1px; }

  /* Drawer panel */
  .mobile-nav {
    position: fixed;
    top: 0; right: 0; bottom: 0;
    width: min(82vw, 320px);
    z-index: 1001;
    background: var(--bg, #fff);
    border-left: 1px solid var(--line, rgba(0,0,0,0.12));
    padding: 4.5rem 1.25rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    overflow-y: auto;
    box-shadow: -8px 0 24px rgba(0,0,0,0.18);
    animation: slide-in 180ms ease-out;
  }
  @keyframes slide-in {
    from { transform: translateX(100%); }
    to   { transform: translateX(0); }
  }
  .mobile-nav a {
    text-decoration: none;
    color: inherit;
    padding: 0.7rem 0.6rem;
    border-radius: 0.4rem;
    font-family: var(--mono, monospace);
    font-size: 0.95rem;
  }
  .mobile-nav a:hover { background: var(--bg-2, rgba(0,0,0,0.04)); }
  .mobile-nav hr {
    border: 0;
    border-top: 1px solid var(--line, rgba(0,0,0,0.12));
    margin: 0.4rem 0;
  }
  .mobile-nav .cta {
    background: var(--ink, #111);
    color: var(--bg, #fff);
    text-align: center;
    margin-top: 0.4rem;
  }
  .mobile-nav-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.45);
    z-index: 1000;
    animation: fade-in 180ms ease-out;
    appearance: none;
    border: 0;
    padding: 0;
    cursor: pointer;
  }
  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  /* Below the existing 900 px breakpoint, swap inline links for the
     hamburger. The :global() targets exist in app.css; we reach them
     here so the rule travels with the component. */
  @media (max-width: 900px) {
    .nav-burger { display: inline-flex; }
    :global(nav.top .nav-links) { display: none; }
    :global(nav.top .nav-auth)  { display: none; }
  }
</style>
