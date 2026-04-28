<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';

  let email = $state('');
  let password = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: Event) {
    event.preventDefault();
    error = null;
    busy = true;
    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email, password })
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        error = (body as { message?: string }).message ?? `login failed (${res.status})`;
        return;
      }
      const next = $page.url.searchParams.get('next') ?? '/dashboard';
      await goto(next, { invalidateAll: true });
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head>
  <title>Sign in — UCFP</title>
</svelte:head>

<h1>Sign in</h1>
<p class="lede-sm">Welcome back. Use your email and password to continue.</p>

<form onsubmit={submit}>
  <label>
    <span>Email</span>
    <input
      type="email"
      autocomplete="email"
      required
      bind:value={email}
      disabled={busy}
    />
  </label>
  <label>
    <span>Password</span>
    <input
      type="password"
      autocomplete="current-password"
      required
      bind:value={password}
      disabled={busy}
    />
  </label>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <button type="submit" class="btn" disabled={busy}>
    {busy ? 'Signing in…' : 'Sign in →'}
  </button>
</form>

<p class="alt">
  No account? <a href="/signup">Create one →</a>
</p>

<style>
  h1 {
    font-family: var(--sans);
    font-weight: 400;
    font-size: 32px;
    letter-spacing: -0.02em;
    margin: 0 0 8px;
  }
  .lede-sm {
    color: var(--ink-2);
    font-size: 14px;
    margin: 0 0 24px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label span {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }
  input {
    background: var(--bg);
    border: 1px solid var(--line-strong);
    padding: 12px 14px;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--ink);
  }
  input:focus-visible {
    outline: 2px solid var(--accent-ink);
    outline-offset: 2px;
  }
  .error {
    color: oklch(0.55 0.15 25);
    font-family: var(--mono);
    font-size: 12px;
    margin: 0;
  }
  .btn {
    margin-top: 8px;
  }
  .alt {
    margin: 24px 0 0;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    text-align: center;
  }
  .alt a {
    color: var(--ink);
  }
</style>
