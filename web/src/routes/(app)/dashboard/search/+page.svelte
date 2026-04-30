<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import type { Modality, QueryHit, RecordHistoryEntry } from '$lib/types/api';
  import { buildResampledAudioForm } from '$lib/utils/audioResample';
  import { createRecordHistory } from '$lib/stores/recordHistory.svelte';

  const history = createRecordHistory();
  // Lookup table from record_id → saved bookmark, so hits with a known
  // record can show their label + hex thumbnail instead of just a u64.
  const historyById = $derived.by(() => {
    const m = new Map<string, RecordHistoryEntry>();
    for (const e of history.entries) m.set(e.recordId, e);
    return m;
  });

  // Same byte→colour map as the records page so the thumbnails match.
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

  // Algorithms that produce a dense embedding (must match playground).
  const SEMANTIC_ALGS: Record<Modality, string[]> = {
    text:  ['semantic-local', 'semantic-openai', 'semantic-voyage', 'semantic-cohere'],
    image: ['semantic'],
    audio: ['neural']
  };

  let modality  = $state<Modality>('text');
  let algorithm = $state<string>('semantic-local');
  let k         = $state<number>(10);

  // Source mode
  let mode = $state<'compute'|'paste'>('compute');

  // Compute mode inputs
  let text     = $state('');
  let file     = $state<File | null>(null);
  let modelId  = $state('');
  let apiKey   = $state('');

  // Paste mode input
  let vectorText = $state('');

  // Results
  let busy        = $state(false);
  let error       = $state<string | null>(null);
  let hits        = $state<QueryHit[]>([]);
  let lastVecLen  = $state<number | null>(null);
  let sourceLabel = $state<string | null>(null);

  function defaultAlg(m: Modality): string {
    return SEMANTIC_ALGS[m][0];
  }

  // Pick up a sessionStorage handoff from the playground (Find similar).
  onMount(() => {
    // URL-driven defaults take precedence so the back button preserves state.
    const sp = $page.url.searchParams;
    const mp = sp.get('modality');
    if (mp === 'text' || mp === 'image' || mp === 'audio') modality = mp;
    const ap = sp.get('algorithm');
    if (ap && SEMANTIC_ALGS[modality].includes(ap)) algorithm = ap;
    else algorithm = defaultAlg(modality);

    try {
      const raw = sessionStorage.getItem('ucfp:search:handoff');
      if (raw) {
        const h = JSON.parse(raw) as { modality: Modality; algorithm: string; vector: number[]; sourceLabel?: string };
        sessionStorage.removeItem('ucfp:search:handoff');
        if (Array.isArray(h.vector) && h.vector.length > 0) {
          modality = h.modality;
          algorithm = h.algorithm;
          mode = 'paste';
          vectorText = JSON.stringify(h.vector);
          sourceLabel = h.sourceLabel ?? null;
          // Auto-run the search since we have everything we need.
          void runSearch();
        }
      }
    } catch { /* ignore */ }
  });

  function onModalityChange(m: Modality) {
    modality = m;
    if (!SEMANTIC_ALGS[m].includes(algorithm)) algorithm = defaultAlg(m);
  }

  function parseVector(s: string): number[] | null {
    const t = s.trim();
    if (!t) return null;
    try {
      // JSON array or comma-separated bare list — both supported.
      const arr = t.startsWith('[') ? JSON.parse(t) : t.split(/[,\s]+/).map(Number);
      if (!Array.isArray(arr)) return null;
      const out: number[] = [];
      for (const v of arr) {
        const n = Number(v);
        if (!Number.isFinite(n)) return null;
        out.push(n);
      }
      return out.length ? out : null;
    } catch {
      return null;
    }
  }

  async function computeEmbedding(): Promise<number[] | null> {
    if (modality === 'text') {
      if (!text.trim()) { error = 'Enter some text to fingerprint.'; return null; }
    } else {
      if (!file) { error = 'Drop a file to fingerprint.'; return null; }
    }
    let url = `/api/fingerprint?algorithm=${encodeURIComponent(algorithm)}&return_embedding=1`;
    if (modelId.trim()) url += `&model_id=${encodeURIComponent(modelId.trim())}`;
    if (apiKey.trim())  url += `&api_key=${encodeURIComponent(apiKey.trim())}`;

    let body: BodyInit;
    let headers: Record<string,string> = {};
    if (modality === 'text') {
      body = text;
      headers['content-type'] = 'text/plain; charset=utf-8';
    } else if (modality === 'audio') {
      // Upstream `/v1/ingest/audio/...` requires raw f32 LE samples + a
      // sample_rate query param. Decode + resample in the browser.
      const built = await buildResampledAudioForm(file as File, algorithm);
      body = built.form;
    } else {
      const fd = new FormData();
      fd.set('file', file as File);
      body = fd;
    }
    const res = await fetch(url, { method: 'POST', body, headers });
    if (!res.ok) {
      error = `Fingerprint failed: ${res.status} ${await res.text().catch(() => '')}`.slice(0, 240);
      return null;
    }
    const data = (await res.json()) as { embedding?: number[]; has_embedding?: boolean };
    if (!Array.isArray(data.embedding) || data.embedding.length === 0) {
      error = 'Backend did not return an embedding for this algorithm. Check that the model produces one.';
      return null;
    }
    return data.embedding;
  }

  async function runSearch() {
    if (busy) return;
    error = null; hits = []; lastVecLen = null;
    busy = true;
    try {
      let vec: number[] | null;
      if (mode === 'compute') {
        vec = await computeEmbedding();
        if (!vec) return;
        sourceLabel = modality === 'text' ? text.trim().slice(0, 60) : (file?.name ?? '');
      } else {
        vec = parseVector(vectorText);
        if (!vec) {
          error = 'Could not parse vector — paste a JSON array or comma-separated floats.';
          return;
        }
      }
      lastVecLen = vec.length;

      const res = await fetch('/api/search', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ modality, k, vector: vec })
      });
      if (res.status === 401) { error = 'Sign in to search.'; return; }
      if (res.status === 503) { error = 'Backend not configured.'; return; }
      if (!res.ok) {
        error = `Search failed: ${res.status} ${(await res.text()).slice(0, 200)}`;
        return;
      }
      const data = (await res.json()) as { hits: QueryHit[] };
      hits = data.hits ?? [];
    } catch (e) {
      error = `Network error: ${(e as Error).message}`;
    } finally {
      busy = false;
    }
  }

  function pickFile(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (f) file = f;
  }

  const NEEDS_API_KEY = new Set(['semantic-openai','semantic-voyage','semantic-cohere']);
  const NEEDS_MODEL   = new Set(['semantic-local','semantic','neural']);
</script>

<div class="srch-wrap">
  <div class="srch-head">
    <h1 class="srch-title">Similarity Search</h1>
    <p class="srch-sub">
      Run kNN over saved fingerprints. The query vector comes from a freshly-computed
      semantic fingerprint, or from a vector you paste directly.
    </p>
  </div>

  {#if sourceLabel}
    <p class="srch-handoff">
      Vector handoff from playground: <strong>{sourceLabel}</strong>
    </p>
  {/if}

  <!-- ── controls row ────────────────────────────────────────────────── -->
  <div class="srch-row">
    <label class="ctrl">
      <span>Modality</span>
      <select bind:value={modality} onchange={() => onModalityChange(modality)}>
        <option value="text">Text</option>
        <option value="image">Image</option>
        <option value="audio">Audio</option>
      </select>
    </label>
    <label class="ctrl">
      <span>Algorithm</span>
      <select bind:value={algorithm}>
        {#each SEMANTIC_ALGS[modality] as a}
          <option value={a}>{a}</option>
        {/each}
      </select>
    </label>
    <label class="ctrl">
      <span>k (top-k)</span>
      <input type="number" min="1" max="100" bind:value={k} />
    </label>
  </div>

  <!-- ── source mode ─────────────────────────────────────────────────── -->
  <div class="srch-mode" role="tablist">
    <button role="tab" class="mode-tab" aria-selected={mode === 'compute'}
      class:active={mode === 'compute'} onclick={() => { mode = 'compute'; }}>
      Compute now
    </button>
    <button role="tab" class="mode-tab" aria-selected={mode === 'paste'}
      class:active={mode === 'paste'} onclick={() => { mode = 'paste'; }}>
      Paste vector
    </button>
  </div>

  {#if mode === 'compute'}
    <div class="srch-pane">
      {#if modality === 'text'}
        <textarea class="srch-textarea" bind:value={text} rows={4}
          placeholder="Text to fingerprint and search…"></textarea>
      {:else}
        <input type="file" accept={modality === 'image' ? 'image/*' : 'audio/*'}
          onchange={pickFile} class="srch-file" />
        {#if file}<p class="srch-hint mono">{file.name} — {(file.size/1024).toFixed(1)} KB</p>{/if}
      {/if}

      {#if NEEDS_MODEL.has(algorithm)}
        <label class="ctrl">
          <span>Model path / ID</span>
          <input type="text" bind:value={modelId}
            placeholder={algorithm === 'neural' ? '/models/audio.onnx' :
                        algorithm === 'semantic' ? '/models/clip.onnx' :
                        'sentence-transformers/all-MiniLM-L6-v2'} />
        </label>
      {/if}
      {#if NEEDS_API_KEY.has(algorithm)}
        <label class="ctrl">
          <span>API key</span>
          <input type="password" bind:value={apiKey} placeholder="sk-…" />
        </label>
      {/if}
    </div>
  {:else}
    <div class="srch-pane">
      <label class="ctrl">
        <span>Vector (JSON array or comma-separated floats)</span>
        <textarea class="srch-textarea" bind:value={vectorText} rows={4}
          placeholder="[0.12, -0.34, 0.56, …] or 0.12, -0.34, 0.56, …"></textarea>
      </label>
    </div>
  {/if}

  {#if error}
    <p class="srch-error" role="alert">{error}</p>
  {/if}

  <button class="run-btn" onclick={runSearch} disabled={busy} aria-busy={busy}>
    {busy ? 'Searching…' : 'Run search'}
  </button>

  <!-- ── hits ────────────────────────────────────────────────────────── -->
  {#if lastVecLen !== null}
    <p class="srch-hint mono">Query vector length: {lastVecLen}</p>
  {/if}
  {#if hits.length > 0}
    {@const scores = hits.map((h) => h.score)}
    {@const top    = scores[0]}
    {@const tail   = scores[scores.length - 1]}
    {@const span   = Math.max(1e-6, top - tail)}
    {@const median = scores[Math.floor(scores.length / 2)]}

    <!-- Score-distribution strip — one notch per hit, x = rank, height = score. -->
    <div class="dist">
      <div class="dist-meta mono">
        <span><strong>top</strong> {top.toFixed(4)}</span>
        <span><strong>median</strong> {median.toFixed(4)}</span>
        <span><strong>tail</strong> {tail.toFixed(4)}</span>
        <span><strong>spread</strong> {span.toFixed(4)}</span>
        <span class="dist-count">{hits.length} {hits.length === 1 ? 'hit' : 'hits'}</span>
      </div>
      <svg viewBox="0 0 100 28" preserveAspectRatio="none" class="dist-svg" role="img"
           aria-label="Score distribution across {hits.length} hits">
        <line x1="0" y1="14" x2="100" y2="14" stroke="var(--ink)" stroke-width="0.15" opacity="0.3" />
        {#each scores as s, i}
          {@const x = (i / Math.max(1, hits.length - 1)) * 100}
          {@const norm = (s - tail) / span}
          {@const h = Math.max(1, norm * 24)}
          <rect x={Math.min(99, x - 0.4)} y={26 - h} width="0.8" height={h}
                fill="var(--accent-ink, oklch(0.55 0.18 240))" />
        {/each}
      </svg>
    </div>

    <ol class="hits">
      {#each hits as h, i}
        {@const norm = (h.score - tail) / span}
        {@const known = historyById.get(String(h.record_id))}
        <li class="hit" class:known>
          <span class="hit-rank mono">#{i+1}</span>
          {#if known}
            <div class="hit-thumb" aria-hidden="true">
              {#each hexTiles(known.fingerprintHex, 16) as t}
                <span class="hit-tile" style="background:{t}"></span>
              {/each}
            </div>
          {:else}
            <span class="hit-thumb-placeholder" aria-hidden="true">⬡</span>
          {/if}
          <div class="hit-meta">
            {#if known}
              <a class="hit-label" href={`/dashboard/records?lookup=${h.record_id}`}>{known.label || '(no label)'}</a>
              <span class="hit-id-sub mono">
                <span class="hit-mod {known.modality}">{known.modality}</span>
                {known.algorithm} · id {String(h.record_id).slice(-10)}
              </span>
            {:else}
              <a class="hit-label mono" href={`/dashboard/records?lookup=${h.record_id}`}>{h.record_id}</a>
              <span class="hit-id-sub mono">unsaved · {h.source}</span>
            {/if}
          </div>
          <span class="hit-source mono">{h.source}</span>
          <span class="hit-score mono">{h.score.toFixed(4)}</span>
          <div class="hit-bar"><div class="hit-bar-fill"
            style="width:{Math.max(2, norm * 100)}%;
                   background:oklch(0.55 0.18 {Math.round(120 + norm * 120)})"></div></div>
        </li>
      {/each}
    </ol>
  {:else if !busy && !error && lastVecLen !== null}
    <div class="srch-empty">
      <span class="empty-icon">⬡</span>
      <p>No matches above similarity threshold.</p>
    </div>
  {/if}
</div>

<style>
  .srch-wrap { display: flex; flex-direction: column; gap: 1rem; max-width: 900px; }
  .srch-title { font-size: 1.25rem; font-weight: 700; margin: 0 0 0.25rem; }
  .srch-sub   { margin: 0; color: var(--ink-2); font-size: 0.85rem; }
  .srch-handoff { margin: 0; padding: 0.4rem 0.6rem; background: color-mix(in oklch, var(--accent-ink) 12%, var(--bg)); border-left: 3px solid var(--accent-ink); font-family: var(--mono); font-size: 0.78rem; color: var(--ink); }

  .srch-row { display: flex; gap: 0.6rem; flex-wrap: wrap; }
  .ctrl { display: flex; flex-direction: column; gap: 3px; font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); flex: 1; min-width: 160px; }
  .ctrl input, .ctrl select, .ctrl textarea {
    font-family: var(--mono); font-size: 0.78rem;
    padding: 5px 8px; border: 1px solid var(--ink);
    background: var(--bg); color: var(--ink); border-radius: 3px;
  }

  .srch-mode { display: flex; gap: 0.4rem; }
  .mode-tab {
    font-family: var(--mono); font-size: 0.75rem;
    padding: 0.35rem 0.8rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink-2);
    border-radius: 3px; cursor: pointer;
  }
  .mode-tab.active { background: var(--ink); color: var(--bg); }

  .srch-pane {
    display: flex; flex-direction: column; gap: 0.6rem;
    padding: 0.85rem; background: var(--bg-2);
    border: 1px solid var(--ink); border-radius: 6px;
  }
  .srch-textarea { font-family: var(--mono); font-size: 0.78rem; resize: vertical; min-height: 90px; }
  .srch-file { font-family: var(--mono); font-size: 0.78rem; }

  .srch-error { font-family: var(--mono); font-size: 0.75rem; color: #b03030; margin: 0; padding: 0.4rem 0.6rem; border: 1px solid currentColor; border-radius: 3px; background: color-mix(in srgb, #b03030 8%, transparent); }

  .run-btn {
    font-family: var(--mono); font-size: 0.82rem;
    padding: 0.55rem 1.2rem; border: 1px solid var(--ink);
    background: var(--ink); color: var(--bg); border-radius: 3px;
    cursor: pointer; align-self: flex-start;
  }
  .run-btn:disabled { opacity: 0.45; cursor: not-allowed; }

  .srch-hint { font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); margin: 0; }
  .mono { font-family: var(--mono); }

  .srch-empty {
    display: flex; flex-direction: column; align-items: center;
    gap: 0.4rem; padding: 1.5rem; color: var(--ink-2);
    border: 1px dashed var(--ink); border-radius: 6px; background: var(--bg-2);
  }
  .empty-icon { font-size: 1.6rem; opacity: 0.4; }

  /* Score-distribution strip across all hits — gives a "shape" of the
     ranking before the user reads any individual row. */
  .dist {
    display: flex; flex-direction: column; gap: 0.3rem;
    padding: 0.6rem 0.75rem;
    background: var(--bg-2); border: 1px solid var(--ink); border-radius: 6px;
  }
  .dist-meta {
    display: flex; gap: 0.85rem; flex-wrap: wrap;
    font-size: 0.65rem; color: var(--ink-2);
  }
  .dist-meta strong { font-weight: 400; text-transform: uppercase; letter-spacing: 0.06em; margin-right: 4px; }
  .dist-count { margin-left: auto; }
  .dist-svg {
    width: 100%; height: 32px; display: block;
    background: var(--bg); border: 1px solid var(--ink); border-radius: 3px;
  }

  .hits { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.3rem; }
  .hit {
    display: grid;
    grid-template-columns: 32px 38px 1fr 70px 80px;
    gap: 0.6rem; align-items: center;
    padding: 0.45rem 0.75rem;
    background: var(--bg-2); border: 1px solid var(--ink); border-radius: 4px;
  }
  .hit.known { border-left-width: 3px; border-left-color: var(--accent-ink, oklch(0.55 0.18 240)); }
  .hit-rank { font-size: 0.7rem; color: var(--ink-2); }
  .hit-thumb {
    display: grid; grid-template-columns: repeat(8, 4px); grid-auto-rows: 4px; gap: 1px;
    padding: 2px; background: var(--ink); border-radius: 3px; flex-shrink: 0;
  }
  .hit-tile { width: 4px; height: 4px; border-radius: 1px; display: block; }
  .hit-thumb-placeholder {
    display: flex; align-items: center; justify-content: center;
    width: 38px; height: 38px;
    background: var(--bg); border: 1px dashed var(--ink); border-radius: 3px;
    color: var(--ink-2); font-size: 1.1rem; opacity: 0.4;
  }
  .hit-meta { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .hit-label { color: var(--ink); text-decoration: none; font-size: 0.85rem; word-break: break-all; }
  .hit-label:hover { text-decoration: underline; }
  .hit-id-sub { font-size: 0.62rem; color: var(--ink-2); display: flex; gap: 0.4rem; align-items: center; }
  .hit-mod { padding: 1px 5px; border-radius: 2px; background: var(--ink); color: var(--bg); font-size: 0.55rem; text-transform: uppercase; letter-spacing: 0.05em; }
  .hit-mod.text  { background: oklch(0.55 0.15 240); }
  .hit-mod.image { background: oklch(0.55 0.15 290); }
  .hit-mod.audio { background: oklch(0.55 0.15 145); }
  .hit-source { font-size: 0.6rem; padding: 1px 5px; background: var(--bg); border: 1px solid var(--ink); border-radius: 3px; color: var(--ink-2); text-align: center; }
  .hit-score { font-size: 0.78rem; color: var(--ink); font-weight: 600; text-align: right; }
  .hit-bar {
    grid-column: 1 / -1;
    height: 3px; background: var(--bg); border-radius: 2px; overflow: hidden;
  }
  .hit-bar-fill { height: 100%; transition: width 0.25s ease; }
</style>
