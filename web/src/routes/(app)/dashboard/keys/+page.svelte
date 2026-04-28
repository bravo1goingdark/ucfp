<script lang="ts">
  import { invalidateAll } from '$app/navigation';
  import DataTable, { type Column } from '$components/DataTable.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Modal from '$components/Modal.svelte';
  import { pushToast } from '$lib/stores/toasts.svelte';
  import type { KeyRow, CreatedKey } from '$lib/types/api';

  let { data } = $props();

  // ── create modal ──────────────────────────────────────────────────────
  let createOpen = $state(false);
  let newName = $state('');
  let creating = $state(false);
  let createError = $state<string | null>(null);
  let createdToken = $state<CreatedKey | null>(null);

  function openCreate() {
    newName = '';
    createError = null;
    createdToken = null;
    createOpen = true;
  }

  async function submitCreate(event: Event) {
    event.preventDefault();
    if (creating) return;
    if (!newName.trim()) {
      createError = 'Give the key a name so you can recognise it later.';
      return;
    }
    creating = true;
    createError = null;
    try {
      const res = await fetch('/api/keys', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ name: newName.trim() })
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        createError = (body as { message?: string }).message ?? `Failed (${res.status}).`;
        return;
      }
      createdToken = (await res.json()) as CreatedKey;
      pushToast({ kind: 'success', message: `Key "${createdToken.name}" created.` });
      await invalidateAll();
    } catch (e) {
      createError = (e as Error).message;
    } finally {
      creating = false;
    }
  }

  async function copyToken() {
    if (!createdToken) return;
    try {
      await navigator.clipboard.writeText(createdToken.token);
      pushToast({ kind: 'success', message: 'Token copied to clipboard.' });
    } catch {
      pushToast({ kind: 'error', message: 'Clipboard write failed — copy manually.' });
    }
  }

  // ── revoke ────────────────────────────────────────────────────────────
  let revoking = $state<string | null>(null);
  async function revoke(row: KeyRow) {
    if (revoking) return;
    if (typeof confirm !== 'undefined' && !confirm(`Revoke key "${row.name}"? This cannot be undone.`)) {
      return;
    }
    revoking = row.id;
    try {
      const res = await fetch(`/api/keys/${row.id}`, { method: 'DELETE' });
      if (!res.ok) {
        pushToast({ kind: 'error', message: `Could not revoke "${row.name}".` });
        return;
      }
      pushToast({ kind: 'success', message: `"${row.name}" revoked.` });
      await invalidateAll();
    } catch {
      pushToast({ kind: 'error', message: 'Network error while revoking.' });
    } finally {
      revoking = null;
    }
  }

  function fmtDate(unixSec: number | null): string {
    if (!unixSec) return '—';
    return new Date(unixSec * 1000).toLocaleDateString();
  }

  function statusOf(row: KeyRow): string {
    return row.revokedAt ? 'revoked' : 'active';
  }

  const columns: Column<KeyRow>[] = [
    { key: 'name', label: 'Name', get: (r) => r.name },
    { key: 'prefix', label: 'Prefix', get: (r) => r.prefix },
    { key: 'created', label: 'Created', get: (r) => fmtDate(r.createdAt) },
    { key: 'lastUsed', label: 'Last used', get: (r) => fmtDate(r.lastUsedAt) },
    { key: 'status', label: 'Status', get: (r) => statusOf(r) },
    {
      key: 'actions',
      label: 'Actions',
      cell: actionsCell
    }
  ];
</script>

{#snippet actionsCell(row: KeyRow)}
  {#if row.revokedAt}
    <span class="muted">—</span>
  {:else}
    <button
      type="button"
      class="link-btn danger"
      onclick={() => revoke(row)}
      disabled={revoking === row.id}
    >{revoking === row.id ? 'Revoking…' : 'Revoke'}</button>
  {/if}
{/snippet}

<svelte:head><title>API keys — UCFP</title></svelte:head>

<section class="dash-section">
  <header class="dash-section-head split">
    <div>
      <h1>API keys</h1>
      <p class="muted">Use a key in the <code>Authorization: Bearer …</code> header.</p>
    </div>
    <button type="button" class="btn alt" onclick={openCreate}>+ Create new key</button>
  </header>

  {#if data.error}
    <p class="error" role="alert">{data.error}</p>
  {/if}

  {#if data.keys.length === 0}
    <EmptyState
      heading="No keys yet"
      description="Create your first API key to start fingerprinting from your app."
    >
      {#snippet cta()}
        <button type="button" class="btn" onclick={openCreate}>Create new key →</button>
      {/snippet}
    </EmptyState>
  {:else}
    <DataTable {columns} rows={data.keys} rowKey={(r) => r.id} caption="Your API keys" />
  {/if}
</section>

<Modal bind:open={createOpen} title="Create new API key">
  {#if createdToken}
    <p>Your new key was created. Copy it now — <strong>this is the only time you will see it</strong>.</p>
    <div class="token-box">
      <code>{createdToken.token}</code>
      <button type="button" class="btn alt" onclick={copyToken}>Copy</button>
    </div>
    <p class="muted small">Store it somewhere safe (a secrets manager, your CI vault, etc.).</p>
    <div class="modal-actions">
      <button
        type="button"
        class="btn"
        onclick={() => {
          createOpen = false;
          createdToken = null;
        }}
      >Done</button>
    </div>
  {:else}
    <form onsubmit={submitCreate}>
      <label>
        <span>Key name</span>
        <input
          type="text"
          required
          maxlength="64"
          placeholder="e.g. production-worker"
          bind:value={newName}
          disabled={creating}
        />
      </label>
      {#if createError}
        <p class="error" role="alert">{createError}</p>
      {/if}
      <div class="modal-actions">
        <button type="button" class="btn alt" onclick={() => (createOpen = false)} disabled={creating}>
          Cancel
        </button>
        <button type="submit" class="btn" disabled={creating}>
          {creating ? 'Creating…' : 'Create key'}
        </button>
      </div>
    </form>
  {/if}
</Modal>
