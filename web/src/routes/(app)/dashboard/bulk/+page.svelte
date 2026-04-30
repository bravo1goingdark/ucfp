<script lang="ts">
  import { createRecordHistory } from '$lib/stores/recordHistory.svelte';
  import { buildResampledAudioForm } from '$lib/utils/audioResample';
  import type { Modality, RecordHistoryEntry } from '$lib/types/api';

  const history = createRecordHistory();
  const CONCURRENCY = 4;

  type Status = 'queued' | 'running' | 'done' | 'error' | 'rate-limited';
  interface QueueItem {
    id: number;
    file: File;
    modality: Modality | null;
    algorithm: string;
    status: Status;
    recordId?: string;
    error?: string;
    bytesIn?: number;
  }

  let nextId = 0;
  let items     = $state<QueueItem[]>([]);
  let dragActive = $state(false);
  let running    = $state(false);
  let modelId    = $state('');
  let apiKey     = $state('');

  const DEFAULT_ALG: Record<Modality, string> = { text: 'minhash', image: 'multi', audio: 'wang' };

  function detectModality(f: File): Modality | null {
    const t = (f.type || '').toLowerCase();
    if (t.startsWith('image/')) return 'image';
    if (t.startsWith('audio/')) return 'audio';
    if (t.startsWith('text/') || t === 'application/json') return 'text';
    // Fallback by extension
    const ext = f.name.split('.').pop()?.toLowerCase() ?? '';
    if (['png','jpg','jpeg','webp','gif','bmp'].includes(ext)) return 'image';
    if (['wav','mp3','ogg','flac','m4a'].includes(ext)) return 'audio';
    if (['txt','md','json','csv'].includes(ext)) return 'text';
    return null;
  }

  function addFiles(fs: FileList | File[]) {
    for (const f of fs) {
      const mod = detectModality(f);
      items = [...items, {
        id: nextId++, file: f, modality: mod,
        algorithm: mod ? DEFAULT_ALG[mod] : 'minhash',
        status: 'queued'
      }];
    }
  }

  function onDrop(e: DragEvent) {
    e.preventDefault(); dragActive = false;
    if (e.dataTransfer?.files) addFiles(e.dataTransfer.files);
  }

  function onPick(e: Event) {
    const fs = (e.currentTarget as HTMLInputElement).files;
    if (fs) addFiles(fs);
  }

  function removeItem(id: number) {
    items = items.filter((x) => x.id !== id);
  }

  function clearDone() {
    items = items.filter((x) => x.status === 'queued' || x.status === 'running');
  }

  // Read file → request body for /api/fingerprint, audio resampled in browser.
  async function buildBody(item: QueueItem): Promise<{ body: BodyInit; contentType?: string; bytesIn: number }> {
    if (item.modality === 'text') {
      const text = await item.file.text();
      return { body: text, contentType: 'text/plain; charset=utf-8', bytesIn: new TextEncoder().encode(text).byteLength };
    }
    if (item.modality === 'audio') {
      const built = await buildResampledAudioForm(item.file, item.algorithm);
      return { body: built.form, bytesIn: built.bytes };
    }
    // Image — passthrough as multipart so the existing parser detects modality.
    const fd = new FormData();
    fd.set('file', item.file);
    return { body: fd, bytesIn: item.file.size };
  }

  async function processOne(item: QueueItem): Promise<void> {
    item.status = 'running';
    items = items.map((x) => x.id === item.id ? item : x);
    try {
      let url = `/api/fingerprint?algorithm=${encodeURIComponent(item.algorithm)}`;
      if (modelId.trim()) url += `&model_id=${encodeURIComponent(modelId.trim())}`;
      if (apiKey.trim())  url += `&api_key=${encodeURIComponent(apiKey.trim())}`;

      const built = await buildBody(item);
      item.bytesIn = built.bytesIn;
      const init: RequestInit = { method: 'POST', body: built.body };
      if (built.contentType) init.headers = { 'content-type': built.contentType };

      const res = await fetch(url, init);
      if (res.status === 429) { item.status = 'rate-limited'; item.error = 'rate limited'; return; }
      if (!res.ok) {
        item.status = 'error';
        item.error = `${res.status}: ${(await res.text()).slice(0, 120)}`;
        return;
      }
      const data = await res.json() as { record_id?: string|number; tenant_id?: number; algorithm?: string; has_embedding?: boolean; fingerprint_hex?: string };
      if (data.record_id != null) {
        item.recordId = String(data.record_id);
        const entry: RecordHistoryEntry = {
          tenantId: typeof data.tenant_id === 'number' ? data.tenant_id : 0,
          recordId: item.recordId,
          label: item.file.name,
          modality: item.modality ?? 'text',
          algorithm: data.algorithm ?? item.algorithm,
          hasEmbedding: Boolean(data.has_embedding),
          fingerprintHex: (data.fingerprint_hex ?? '').slice(0, 64),
          createdAt: Math.floor(Date.now() / 1000)
        };
        history.add(entry);
      }
      item.status = 'done';
    } catch (e) {
      item.status = 'error';
      item.error = (e as Error).message;
    } finally {
      items = items.map((x) => x.id === item.id ? item : x);
    }
  }

  async function runAll() {
    if (running) return;
    running = true;
    try {
      const queue = items.filter((x) => x.status === 'queued' && x.modality);
      let cursor = 0;
      const workers = Array.from({ length: CONCURRENCY }, async () => {
        while (true) {
          const idx = cursor++;
          if (idx >= queue.length) return;
          await processOne(queue[idx]);
        }
      });
      await Promise.all(workers);
    } finally {
      running = false;
    }
  }

  function downloadManifest() {
    const rows = items.filter((x) => x.status === 'done' && x.recordId).map((x) => ({
      filename: x.file.name,
      modality: x.modality,
      algorithm: x.algorithm,
      recordId: x.recordId,
      bytesIn: x.bytesIn
    }));
    const blob = new Blob([JSON.stringify(rows, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = `ucfp-bulk-manifest-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  const counts = $derived.by(() => {
    let queued = 0, running_ = 0, done = 0, errored = 0;
    for (const i of items) {
      if (i.status === 'queued') queued++;
      else if (i.status === 'running') running_++;
      else if (i.status === 'done') done++;
      else errored++;
    }
    return { queued, running: running_, done, errored };
  });
</script>

<div class="bulk-wrap">
  <div class="bulk-head">
    <h1 class="bulk-title">Bulk fingerprinting</h1>
    <p class="bulk-sub">
      Drop multiple files at once — UCFP processes them in parallel (max {CONCURRENCY}),
      saves each to your records, and gives you a JSON manifest at the end.
    </p>
  </div>

  <div class="drop-zone" class:drag-over={dragActive}
    role="button" tabindex="0"
    aria-label="Drop files here"
    ondragover={(e) => { e.preventDefault(); dragActive = true; }}
    ondragleave={() => { dragActive = false; }}
    ondrop={onDrop}
    onclick={() => document.getElementById('bulk-file-input')?.click()}
    onkeydown={(e) => { if (e.key==='Enter'||e.key===' ') document.getElementById('bulk-file-input')?.click(); }}>
    <span class="drop-icon">⬡</span>
    <span>Drop files here or click to browse</span>
    <span class="drop-sub">text · image · audio</span>
  </div>
  <input id="bulk-file-input" type="file" multiple class="sr-only" onchange={onPick} />

  <details class="adv-opts">
    <summary>Default model / API key (used by all queued items)</summary>
    <div class="adv-body">
      <label class="ctrl">
        <span>Model path / ID (semantic-local / image-semantic / neural)</span>
        <input type="text" bind:value={modelId} placeholder="/models/clip.onnx" />
      </label>
      <label class="ctrl">
        <span>API key (semantic-openai / -voyage / -cohere)</span>
        <input type="password" bind:value={apiKey} placeholder="sk-…" />
      </label>
    </div>
  </details>

  {#if items.length > 0}
    <div class="bulk-actions">
      <button class="btn primary" onclick={runAll} disabled={running || counts.queued === 0}>
        {running ? `Running (${counts.running})…` : `Run all (${counts.queued})`}
      </button>
      <button class="btn" onclick={downloadManifest} disabled={counts.done === 0}>
        Download manifest ({counts.done})
      </button>
      <button class="btn" onclick={clearDone} disabled={counts.done === 0 && counts.errored === 0}>
        Clear finished
      </button>
      <span class="bulk-counts mono">
        queued {counts.queued} · running {counts.running} · done {counts.done} · errored {counts.errored}
      </span>
    </div>

    <ul class="queue">
      {#each items as it (it.id)}
        <li class="qitem status-{it.status}">
          <span class="qname mono">{it.file.name}</span>
          <select class="qsel" bind:value={it.modality} disabled={it.status === 'running'}>
            <option value="text">text</option>
            <option value="image">image</option>
            <option value="audio">audio</option>
          </select>
          <select class="qsel" bind:value={it.algorithm} disabled={it.status === 'running'}>
            {#if it.modality === 'text'}
              <option value="minhash">minhash</option>
              <option value="simhash-tf">simhash-tf</option>
              <option value="simhash-idf">simhash-idf</option>
              <option value="lsh">lsh</option>
              <option value="tlsh">tlsh</option>
            {:else if it.modality === 'image'}
              <option value="multi">multi</option>
              <option value="phash">phash</option>
              <option value="dhash">dhash</option>
              <option value="ahash">ahash</option>
            {:else if it.modality === 'audio'}
              <option value="wang">wang</option>
              <option value="panako">panako</option>
              <option value="haitsma">haitsma</option>
            {/if}
          </select>
          <span class="qstatus">{it.status}</span>
          {#if it.recordId}
            <a class="qrec mono" href={`/dashboard/records?lookup=${it.recordId}`}>
              {it.recordId.slice(-12)}
            </a>
          {/if}
          {#if it.error}<span class="qerr mono" title={it.error}>{it.error.slice(0, 40)}</span>{/if}
          <button class="qrm" onclick={() => removeItem(it.id)} title="Remove">×</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .bulk-wrap { display: flex; flex-direction: column; gap: 1rem; }
  .bulk-title { font-size: 1.25rem; font-weight: 700; margin: 0 0 0.25rem; }
  .bulk-sub   { margin: 0; color: var(--ink-2); font-size: 0.85rem; }

  .drop-zone {
    border: 1px dashed var(--ink); border-radius: 6px;
    background: var(--bg-2); min-height: 130px;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 0.4rem; cursor: pointer; transition: border-color 0.15s, background 0.15s;
    color: var(--ink-2); font-size: 0.85rem;
  }
  .drop-zone.drag-over { border-color: var(--accent-ink); background: var(--bg); }
  .drop-zone:focus-visible { outline: 2px solid var(--accent-ink); outline-offset: 2px; }
  .drop-icon { font-size: 2rem; }
  .drop-sub { font-size: 0.7rem; opacity: 0.7; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); border: 0; }

  .adv-opts { border: 1px solid var(--ink); border-radius: 4px; }
  .adv-opts summary { font-family: var(--mono); font-size: 0.72rem; padding: 0.4rem 0.65rem; cursor: pointer; color: var(--ink-2); }
  .adv-body { padding: 0.6rem 0.65rem; display: flex; flex-direction: column; gap: 0.5rem; background: var(--bg-2); }

  .ctrl { display: flex; flex-direction: column; gap: 3px; font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); }
  .ctrl input {
    font-family: var(--mono); font-size: 0.78rem;
    padding: 5px 8px; border: 1px solid var(--ink);
    background: var(--bg); color: var(--ink); border-radius: 3px;
  }

  .bulk-actions { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .btn {
    font-family: var(--mono); font-size: 0.78rem;
    padding: 0.45rem 0.9rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink); border-radius: 3px; cursor: pointer;
  }
  .btn.primary { background: var(--ink); color: var(--bg); }
  .btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .bulk-counts { color: var(--ink-2); font-size: 0.72rem; margin-left: auto; }
  .mono { font-family: var(--mono); }

  .queue { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .qitem {
    display: grid;
    grid-template-columns: 1fr 110px 130px 90px 110px 1fr 24px;
    gap: 0.4rem; align-items: center;
    padding: 0.35rem 0.6rem;
    background: var(--bg-2); border: 1px solid var(--ink); border-radius: 3px;
    font-family: var(--mono); font-size: 0.72rem;
  }
  .qitem.status-running { border-color: var(--accent-ink); }
  .qitem.status-done    { background: color-mix(in oklch, oklch(0.55 0.15 145) 8%, var(--bg-2)); }
  .qitem.status-error,
  .qitem.status-rate-limited { background: color-mix(in srgb, #b03030 6%, var(--bg-2)); }
  .qname { word-break: break-all; }
  .qsel  { font-family: var(--mono); font-size: 0.72rem; padding: 3px 6px; border: 1px solid var(--ink); background: var(--bg); color: var(--ink); border-radius: 3px; }
  .qstatus { color: var(--ink-2); }
  .qrec, .qerr { color: var(--ink); text-decoration: none; }
  .qrec:hover { text-decoration: underline; }
  .qerr { color: #b03030; }
  .qrm  { background: transparent; border: none; color: var(--ink-2); cursor: pointer; font-size: 1rem; }
  .qrm:hover { color: var(--ink); }
</style>
