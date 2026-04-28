<script lang="ts">
  import { goto } from '$app/navigation';

  let email = $state('');
  let password = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: Event) {
    event.preventDefault();
    error = null;
    busy = true;
    try {
      // Turnstile token would be attached here once the widget is mounted.
      // For now we send no token; the server treats it as test-mode-pass
      // when TURNSTILE_SECRET is unset (dev / preview).
      const res = await fetch('/api/auth/signup', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email, password })
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        error = (body as { message?: string }).message ?? `signup failed (${res.status})`;
        return;
      }
      await goto('/dashboard', { invalidateAll: true });
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head>
  <title>Create account — UCFP</title>
</svelte:head>

<h1>Create account</h1>
<p class="lede-sm">Free tier — 50,000 fingerprints / day, no credit card.</p>

<form onsubmit={submit}>
  <label>
    <span>Email</span>
    <input type="email" autocomplete="email" required bind:value={email} disabled={busy} />
  </label>
  <label>
    <span>Password</span>
    <input
      type="password"
      autocomplete="new-password"
      required
      minlength="10"
      bind:value={password}
      disabled={busy}
    />
    <small>Minimum 10 characters.</small>
  </label>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <button type="submit" class="btn" disabled={busy}>
    {busy ? 'Creating account…' : 'Create account →'}
  </button>
</form>

<p class="alt">
  Already have an account? <a href="/login">Sign in →</a>
</p>

<style>
  h1 { font-family: var(--sans); font-weight: 400; font-size: 32px; letter-spacing: -0.02em; margin: 0 0 8px; }
  .lede-sm { color: var(--ink-2); font-size: 14px; margin: 0 0 24px; }
  form { display: flex; flex-direction: column; gap: 16px; }
  label { display: flex; flex-direction: column; gap: 6px; }
  label span { font-family: var(--mono); font-size: 11px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--muted); }
  label small { font-family: var(--mono); font-size: 10px; color: var(--muted); margin-top: 2px; }
  input { background: var(--bg); border: 1px solid var(--line-strong); padding: 12px 14px; font-family: var(--mono); font-size: 13px; color: var(--ink); }
  input:focus-visible { outline: 2px solid var(--accent-ink); outline-offset: 2px; }
  .error { color: oklch(0.55 0.15 25); font-family: var(--mono); font-size: 12px; margin: 0; }
  .btn { margin-top: 8px; }
  .alt { margin: 24px 0 0; font-family: var(--mono); font-size: 12px; color: var(--muted); text-align: center; }
  .alt a { color: var(--ink); }
</style>
