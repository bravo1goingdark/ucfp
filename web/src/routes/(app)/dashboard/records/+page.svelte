<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { createRecordHistory } from '$lib/stores/recordHistory.svelte';
  import { pushToast } from '$lib/stores/toasts.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import type { FingerprintDescription, Modality, RecordHistoryEntry } from '$lib/types/api';

  const history = createRecordHistory();
  // recordId currently in "confirm delete" state (replaces window.confirm).
  let pendingDeleteId = $state<string | null>(null);

  let modalityFilter = $state<'all'|Modality>('all');
  let labelQuery     = $state('');
  let lookupId       = $state('');
  let lookupBusy     = $state(false);
  let lookupError    = $state<string | null>(null);

  // Record currently being viewed (either from history or lookup).
  let viewing        = $state<FingerprintDescription | null>(null);
  let viewingBusy    = $state(false);
  let viewingError   = $state<string | null>(null);

  const filtered = $derived.by(() => {
    const q = labelQuery.trim().toLowerCase();
    return history.entries
      .filter((e) => modalityFilter === 'all' || e.modality === modalityFilter)
      .filter((e) => !q || e.label.toLowerCase().includes(q))
      .slice()
      .reverse(); // newest first
  });

  async function viewRecord(id: string) {
    viewing = null; viewingError = null; viewingBusy = true;
    try {
      const res = await fetch(`/api/records/${encodeURIComponent(id)}`);
      if (res.status === 404) { viewingError = 'Record not found upstream'; return; }
      if (res.status === 401) { viewingError = 'Sign in to view records.'; return; }
      if (!res.ok) { viewingError = `Lookup failed: ${res.status}`; return; }
      viewing = (await res.json()) as FingerprintDescription;
    } catch (e) {
      viewingError = `Network error: ${(e as Error).message}`;
    } finally {
      viewingBusy = false;
    }
  }

  async function confirmDelete(entry: RecordHistoryEntry) {
    pendingDeleteId = null;
    try {
      const res = await fetch(`/api/records/${encodeURIComponent(entry.recordId)}`, { method: 'DELETE' });
      if (res.ok || res.status === 204 || res.status === 404) {
        history.remove(entry.recordId);
        if (viewing && String(viewing.record_id) === entry.recordId) viewing = null;
        pushToast({ kind: 'success', message: `Deleted record ${entry.recordId.slice(-8)}` });
      } else {
        pushToast({ kind: 'error', message: `Delete failed (${res.status})` });
      }
    } catch (e) {
      pushToast({ kind: 'error', message: `Delete failed: ${(e as Error).message}` });
    }
  }

  async function doLookup() {
    const id = lookupId.trim();
    if (!id) return;
    if (!/^\d+$/.test(id)) { lookupError = 'Record id must be a u64 decimal'; return; }
    lookupBusy = true; lookupError = null;
    await viewRecord(id);
    lookupBusy = false;
  }

  function findSimilarFromEntry(entry: RecordHistoryEntry) {
    // We do not have the embedding in storage, so we navigate to search
    // with a hint that the user can re-run the algorithm against a fresh
    // input. The Search page handles the rest.
    void goto(`/dashboard/search?modality=${entry.modality}&algorithm=${entry.algorithm}&hint=${encodeURIComponent(entry.label)}`);
  }

  function formatTime(unix: number): string {
    return new Date(unix * 1000).toISOString().replace('T', ' ').slice(0, 16);
  }

  // Pull `count` byte values from the hex string and turn each into a
  // background colour (hue from byte). Mirrors the playground's hex-grid.
  function hexTiles(hex: string, count: number): string[] {
    const out: string[] = [];
    const max = Math.min(count, Math.floor(hex.length / 2));
    for (let i = 0; i < max; i++) {
      const b = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
      const lightness = 0.45 + (b & 0x3F) / 0x3F * 0.35;
      out.push(`oklch(${lightness.toFixed(3)} 0.16 ${Math.round((b / 255) * 360)}deg)`);
    }
    while (out.length < count) out.push('var(--bg-2)');
    return out;
  }

  // Auto-trigger lookup when arriving with `?lookup=<u64>` (search hits +
  // bulk-result links use this).
  onMount(() => {
    const id = $page.url.searchParams.get('lookup');
    if (id && /^\d+$/.test(id)) {
      lookupId = id;
      void doLookup();
    }
  });
</script>

<div class="rec-wrap">
  <div class="rec-head">
    <h1 class="rec-title">Records</h1>
    <p class="rec-sub">
      Bookmarks of fingerprints saved from the playground. Records live in the
      upstream UCFP backend; this page indexes the ones you've saved here.
    </p>
  </div>

  <!-- ── controls ────────────────────────────────────────────────────── -->
  <div class="rec-controls">
    <label class="ctrl">
      <span>Modality</span>
      <select bind:value={modalityFilter}>
        <option value="all">All</option>
        <option value="text">Text</option>
        <option value="image">Image</option>
        <option value="audio">Audio</option>
      </select>
    </label>
    <label class="ctrl grow">
      <span>Filter by label</span>
      <input type="text" bind:value={labelQuery} placeholder="search labels…" />
    </label>
    <label class="ctrl">
      <span>Lookup by record ID</span>
      <input type="text" bind:value={lookupId} placeholder="u64 decimal" />
    </label>
    <button class="btn" onclick={doLookup} disabled={lookupBusy}>
      {lookupBusy ? 'Loading…' : 'Lookup'}
    </button>
  </div>

  {#if lookupError}
    <p class="rec-error" role="alert">{lookupError}</p>
  {/if}

  <!-- ── list ────────────────────────────────────────────────────────── -->
  {#if filtered.length === 0}
    {#if labelQuery.trim() || modalityFilter !== 'all'}
      <EmptyState heading="No matches" description="Try clearing the modality filter or label search." />
    {:else}
      <EmptyState heading="No saved records yet"
        description="Open the Playground, run a fingerprint, then click 'Save to records' — the bookmark will appear here.">
        {#snippet cta()}
          <a href="/dashboard/playground" class="btn primary-btn">Open Playground</a>
        {/snippet}
      </EmptyState>
    {/if}
  {:else}
    <ul class="rec-list">
      {#each filtered as e (e.recordId)}
        {@const tiles = hexTiles(e.fingerprintHex, 32)}
        {@const pending = pendingDeleteId === e.recordId}
        <li class="rec-item" class:pending>
          <div class="rec-item-head">
            <span class="rec-mod {e.modality}">{e.modality}</span>
            <span class="rec-alg">{e.algorithm}</span>
            <span class="rec-time">{formatTime(e.createdAt)}</span>
          </div>
          <div class="rec-body">
            <!-- Mini hex thumbnail — 4 rows × 8 cols, derived from fingerprintHex bytes. -->
            <div class="rec-thumb" aria-hidden="true">
              {#each tiles as t}
                <span class="rec-tile" style="background:{t}"></span>
              {/each}
            </div>
            <div class="rec-text">
              <div class="rec-label">{e.label || '(no label)'}</div>
              <div class="rec-meta">
                <span class="rec-id mono" title={e.recordId}>id: {e.recordId.slice(-12)}</span>
                <span class="rec-hex mono" title={e.fingerprintHex}>{e.fingerprintHex.slice(0, 24)}…</span>
                {#if e.hasEmbedding}<span class="rec-pill">embedding</span>{/if}
              </div>
            </div>
          </div>
          <div class="rec-actions">
            {#if pending}
              <span class="confirm-msg">Delete this record?</span>
              <button class="action-btn danger" onclick={() => confirmDelete(e)}>Yes, delete</button>
              <button class="action-btn" onclick={() => { pendingDeleteId = null; }}>Cancel</button>
            {:else}
              <button class="action-btn" onclick={() => viewRecord(e.recordId)}>View</button>
              <button class="action-btn" onclick={() => { pendingDeleteId = e.recordId; }}>Delete</button>
              {#if e.hasEmbedding}
                <button class="action-btn" onclick={() => findSimilarFromEntry(e)}>Find similar</button>
              {/if}
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <!-- ── viewer ──────────────────────────────────────────────────────── -->
  {#if viewingBusy || viewing || viewingError}
    <section class="viewer">
      <h2 class="viewer-title">Record detail</h2>
      {#if viewingBusy}
        <p class="hint">Loading…</p>
      {:else if viewingError}
        <p class="rec-error" role="alert">{viewingError}</p>
      {:else if viewing}
        <dl class="viewer-grid">
          <dt>Tenant</dt><dd class="mono">{viewing.tenant_id}</dd>
          <dt>Record ID</dt><dd class="mono">{viewing.record_id}</dd>
          <dt>Modality</dt><dd>{viewing.modality}</dd>
          <dt>Algorithm</dt><dd>{viewing.algorithm}</dd>
          <dt>Format version</dt><dd>{viewing.format_version}</dd>
          <dt>Config hash</dt><dd class="mono">0x{Number(viewing.config_hash).toString(16)}</dd>
          <dt>Fingerprint bytes</dt><dd>{viewing.fingerprint_bytes}</dd>
          <dt>Embedding</dt>
          <dd>{viewing.has_embedding ? `${viewing.embedding_dim}-d` : 'none'}</dd>
          {#if viewing.model_id}
            <dt>Model</dt><dd class="mono">{viewing.model_id}</dd>
          {/if}
          <dt>Metadata bytes</dt><dd>{viewing.metadata_bytes}</dd>
        </dl>
      {/if}
    </section>
  {/if}
</div>

<style>
  .rec-wrap { display: flex; flex-direction: column; gap: 1.25rem; }
  .rec-title { font-size: 1.25rem; font-weight: 700; margin: 0 0 0.25rem; }
  .rec-sub   { margin: 0; color: var(--ink-2); font-size: 0.85rem; }

  .rec-controls {
    display: flex; gap: 0.6rem; align-items: end; flex-wrap: wrap;
    padding: 0.75rem; background: var(--bg-2);
    border: 1px solid var(--ink); border-radius: 6px;
  }
  .ctrl { display: flex; flex-direction: column; gap: 3px; font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); min-width: 140px; }
  .ctrl.grow { flex: 1; min-width: 200px; }
  .ctrl input, .ctrl select {
    font-family: var(--mono); font-size: 0.78rem;
    padding: 5px 8px; border: 1px solid var(--ink);
    background: var(--bg); color: var(--ink); border-radius: 3px;
  }
  .btn {
    font-family: var(--mono); font-size: 0.78rem;
    padding: 0.45rem 0.9rem; border: 1px solid var(--ink);
    background: var(--ink); color: var(--bg); border-radius: 3px;
    cursor: pointer; align-self: end; height: 32px;
  }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }

  .hint { font-size: 0.78rem; margin: 0; }

  .rec-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .rec-item {
    padding: 0.65rem 0.85rem; background: var(--bg-2);
    border: 1px solid var(--ink); border-radius: 4px;
    display: flex; flex-direction: column; gap: 0.4rem;
  }
  .rec-item-head { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; font-family: var(--mono); font-size: 0.7rem; }
  .rec-mod { padding: 2px 6px; border-radius: 3px; background: var(--ink); color: var(--bg); text-transform: uppercase; font-size: 0.62rem; letter-spacing: 0.05em; }
  .rec-mod.text  { background: oklch(0.55 0.15 240); }
  .rec-mod.image { background: oklch(0.55 0.15 290); }
  .rec-mod.audio { background: oklch(0.55 0.15 145); }
  .rec-alg { color: var(--ink-2); }
  .rec-time { margin-left: auto; color: var(--ink-2); font-size: 0.65rem; }
  .rec-label { font-size: 0.85rem; color: var(--ink); }
  .rec-meta { display: flex; gap: 0.5rem; flex-wrap: wrap; font-family: var(--mono); font-size: 0.65rem; color: var(--ink-2); }
  .rec-pill { padding: 1px 5px; background: var(--bg); border: 1px solid var(--ink); border-radius: 3px; }
  .rec-body { display: flex; gap: 0.7rem; align-items: center; }
  .rec-thumb {
    flex-shrink: 0;
    display: grid; grid-template-columns: repeat(8, 8px); grid-auto-rows: 8px; gap: 1px;
    padding: 3px; background: var(--ink); border-radius: 3px;
  }
  .rec-tile { display: block; width: 8px; height: 8px; border-radius: 1px; }
  .rec-text { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; flex: 1; }

  .rec-actions { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .confirm-msg {
    font-family: var(--mono); font-size: 0.72rem; color: var(--ink-2);
    margin-right: 0.25rem;
  }
  .action-btn {
    font-family: var(--mono); font-size: 0.7rem;
    padding: 0.3rem 0.6rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink); border-radius: 3px; cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .action-btn:hover { background: var(--bg); }
  .action-btn.danger {
    background: oklch(0.55 0.18 30); color: var(--bg); border-color: oklch(0.55 0.18 30);
  }
  .action-btn.danger:hover { background: oklch(0.45 0.18 30); }
  .rec-item.pending { border-color: oklch(0.55 0.18 30); }

  .primary-btn {
    background: var(--ink); color: var(--bg); border: 1px solid var(--ink);
    padding: 0.5rem 1rem; border-radius: 3px;
    font-family: var(--mono); font-size: 0.78rem;
    text-decoration: none; display: inline-block;
  }
  .primary-btn:hover { opacity: 0.85; }

  .rec-error { font-family: var(--mono); font-size: 0.75rem; color: #b03030; margin: 0; padding: 0.4rem 0.6rem; border: 1px solid currentColor; border-radius: 3px; background: color-mix(in srgb, #b03030 8%, transparent); }

  .viewer { padding: 0.85rem 1rem; background: var(--bg-2); border: 1px solid var(--ink); border-radius: 6px; }
  .viewer-title { font-family: var(--mono); font-size: 0.85rem; margin: 0 0 0.6rem; }
  .viewer-grid { display: grid; grid-template-columns: 140px 1fr; gap: 0.35rem 0.75rem; margin: 0; font-family: var(--mono); font-size: 0.78rem; }
  .viewer-grid dt { color: var(--ink-2); text-transform: uppercase; font-size: 0.62rem; letter-spacing: 0.05em; align-self: center; }
  .viewer-grid dd { margin: 0; color: var(--ink); }
  .mono { font-family: var(--mono); word-break: break-all; }
</style>
