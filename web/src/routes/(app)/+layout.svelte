<script lang="ts">
  import { page } from '$app/stores';
  import { afterNavigate, goto, invalidateAll } from '$app/navigation';
  import Sidebar, { type NavItem } from '$components/Sidebar.svelte';
  import Breadcrumb, { type Crumb } from '$components/Breadcrumb.svelte';
  import Toast from '$components/Toast.svelte';
  import Seo from '$lib/components/Seo.svelte';
  import { pushToast } from '$lib/stores/toasts.svelte';

  let { data, children } = $props();

  const navItems: NavItem[] = [
    { label: 'Dashboard', href: '/dashboard' },
    { section: 'Workspace', label: 'Playground', href: '/dashboard/playground' },
    { section: 'Workspace', label: 'Bulk',       href: '/dashboard/bulk' },
    { section: 'Workspace', label: 'Records',    href: '/dashboard/records' },
    { section: 'Workspace', label: 'Search',     href: '/dashboard/search' },
    { section: 'Account',   label: 'Keys',       href: '/dashboard/keys' },
    { section: 'Account',   label: 'Usage',      href: '/dashboard/usage' },
    { section: 'Help',      label: 'Docs',       href: '/docs' }
  ];

  const crumbs = $derived.by<Crumb[]>(() => {
    const path = $page.url.pathname;
    if (path === '/dashboard') return [{ label: 'Dashboard' }];
    if (path.startsWith('/dashboard/keys')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Keys' }];
    }
    if (path.startsWith('/dashboard/usage')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Usage' }];
    }
    if (path.startsWith('/dashboard/playground')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Playground' }];
    }
    if (path.startsWith('/dashboard/records')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Records' }];
    }
    if (path.startsWith('/dashboard/search')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Search' }];
    }
    if (path.startsWith('/dashboard/bulk')) {
      return [{ label: 'Dashboard', href: '/dashboard' }, { label: 'Bulk upload' }];
    }
    return [{ label: 'Dashboard', href: '/dashboard' }];
  });

  // Mobile sidebar drawer toggle. Hidden by default; .dash-burger is
  // only visible on <820 px via the rule in app.css.
  let navOpen = $state(false);
  function closeNav() { navOpen = false; }
  // Close the drawer whenever the route changes (covers nav-link taps
  // inside it without smearing click handlers across the wrapper div).
  afterNavigate(() => { navOpen = false; });

  let loggingOut = $state(false);
  async function logout() {
    if (loggingOut) return;
    loggingOut = true;
    try {
      const res = await fetch('/api/auth/logout', { method: 'POST' });
      if (!res.ok) {
        pushToast({ kind: 'error', message: 'Logout failed.' });
        return;
      }
      await invalidateAll();
      await goto('/login');
    } catch {
      pushToast({ kind: 'error', message: 'Network error during logout.' });
    } finally {
      loggingOut = false;
    }
  }
</script>

<!-- Every dashboard route is auth-walled and tenant-private — keep
     crawlers out by default. Individual pages may still set their own
     <title> via <svelte:head> for the browser tab. -->
<Seo
  title="Dashboard"
  description="UCFP workspace — playground, records, search, keys, and usage."
  noindex
/>

<div class="dashboard-shell" class:nav-open={navOpen}>
  <header class="dash-top">
    <button
      type="button"
      class="dash-burger"
      aria-label={navOpen ? 'Close navigation' : 'Open navigation'}
      aria-expanded={navOpen}
      aria-controls="dash-nav-drawer"
      onclick={() => (navOpen = !navOpen)}
    >
      {#if navOpen}
        <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
          <path d="M5 5l10 10M15 5L5 15" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      {:else}
        <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
          <path d="M3 6h14M3 10h14M3 14h14" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      {/if}
    </button>

    <a class="brand" href="/dashboard" aria-label="UCFP dashboard">
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

    <Breadcrumb {crumbs} />

    <div class="dash-top-right">
      <span class="user-chip" title="Signed in as {data.user.email}">{data.user.email}</span>
      <button type="button" class="logout-btn" onclick={logout} disabled={loggingOut}>
        {loggingOut ? 'Signing out…' : 'Logout'}
      </button>
    </div>
  </header>

  <div class="dash-body">
    {#if navOpen}
      <button type="button" class="dash-nav-backdrop" aria-label="Close navigation" onclick={closeNav}></button>
    {/if}
    <div id="dash-nav-drawer" class="dash-nav-drawer">
      <Sidebar items={navItems} />
    </div>
    <main class="dash-main" id="main">
      {@render children()}
    </main>
  </div>
</div>

<Toast />

<style>
  /* Hamburger only appears on the narrow layout. */
  .dash-burger {
    display: none;
    appearance: none;
    border: 1px solid var(--ink);
    background: transparent;
    color: inherit;
    width: 36px;
    height: 36px;
    border-radius: 0.4rem;
    cursor: pointer;
    align-items: center;
    justify-content: center;
  }
  .dash-burger:hover { background: var(--bg-2, rgba(0,0,0,0.04)); }
  .dash-burger:focus-visible { outline: 2px solid var(--accent, #6ad); outline-offset: 1px; }

  /* Below the existing 820 px breakpoint, the sidebar slides in as a
     fixed-position drawer instead of horizontally scrunching. */
  @media (max-width: 820px) {
    .dash-burger { display: inline-flex; }

    .dash-nav-drawer {
      position: fixed;
      top: 0; bottom: 0; left: 0;
      width: min(78vw, 280px);
      z-index: 1001;
      background: var(--bg);
      border-right: 1px solid var(--line);
      padding: 4.5rem 0.6rem 1rem;
      overflow-y: auto;
      transform: translateX(-100%);
      transition: transform 180ms ease-out;
      box-shadow: 8px 0 24px rgba(0,0,0,0.18);
    }
    :global(.dashboard-shell.nav-open) .dash-nav-drawer { transform: translateX(0); }

    /* Inside the drawer, override the horizontal-scrunch sidebar style
       app.css applies at this breakpoint so the menu reads as a column. */
    :global(.dash-nav-drawer .sidebar) { position: static; }
    :global(.dash-nav-drawer .sidebar ul) {
      flex-direction: column;
      flex-wrap: nowrap;
      border-bottom: 0;
      padding-bottom: 0;
    }
    :global(.dash-nav-drawer .sidebar a) {
      border-left: 2px solid transparent;
      border-bottom: 0;
      padding: 0.65rem 0.85rem;
    }
    :global(.dash-nav-drawer .sidebar a.active) {
      border-left-color: var(--accent-ink);
      border-bottom-color: transparent;
    }

    .dash-nav-backdrop {
      position: fixed;
      inset: 0;
      background: rgba(0,0,0,0.45);
      z-index: 1000;
      animation: dash-fade-in 180ms ease-out;
      appearance: none;
      border: 0;
      padding: 0;
      cursor: pointer;
    }
    @keyframes dash-fade-in {
      from { opacity: 0; }
      to   { opacity: 1; }
    }

    /* Reclaim the grid column the sidebar used to occupy. */
    :global(.dashboard-shell .dash-body) {
      grid-template-columns: 1fr;
    }
  }
</style>
