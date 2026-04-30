<script lang="ts">
  import { page } from '$app/stores';
  import { goto, invalidateAll } from '$app/navigation';
  import Sidebar, { type NavItem } from '$components/Sidebar.svelte';
  import Breadcrumb, { type Crumb } from '$components/Breadcrumb.svelte';
  import Toast from '$components/Toast.svelte';
  import { pushToast } from '$lib/stores/toasts.svelte';

  let { data, children } = $props();

  const navItems: NavItem[] = [
    { label: 'Dashboard', href: '/dashboard' },
    { label: 'Keys', href: '/dashboard/keys' },
    { label: 'Usage', href: '/dashboard/usage' },
    { label: 'Playground', href: '/dashboard/playground' },
    { label: 'Records', href: '/dashboard/records' },
    { label: 'Search', href: '/dashboard/search' },
    { label: 'Bulk', href: '/dashboard/bulk' },
    { label: 'Docs', href: '/docs' }
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

<div class="dashboard-shell">
  <header class="dash-top">
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
    <Sidebar items={navItems} />
    <main class="dash-main" id="main">
      {@render children()}
    </main>
  </div>
</div>

<Toast />
