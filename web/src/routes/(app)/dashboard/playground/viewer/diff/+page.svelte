<!--
  Fullbleed A/B diff viewer.

  URL: ?a=<input_id_a>&b=<input_id_b>&algorithm=X&modality=Y[&sample_rate=Z]

  Loads two fingerprints in parallel (same algorithm) and renders the
  appropriate diff visualization: AlgorithmView with diffAgainst for
  algorithms that support it, BitDiffStrip for the universal XOR view.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import { apiFetch } from '$lib/utils/apiFetch.svelte';
  import AlgorithmView from '$components/charts/AlgorithmView.svelte';
  import { hasAlgorithmView } from '$components/charts/algorithmView';
  import BitDiffStrip from '$components/charts/BitDiffStrip.svelte';

  function hexToBytes(hex: string): Uint8Array {
    if (hex.length % 2 !== 0) return new Uint8Array(0);
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) {
      out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  let bytesA = $state<Uint8Array | null>(null);
  let bytesB = $state<Uint8Array | null>(null);
  let algorithm = $state<string>('');
  let loading = $state(true);
  let err = $state<string | null>(null);

  function popcount(x: number): number {
    let v = x;
    v = v - ((v >> 1) & 0x55);
    v = (v & 0x33) + ((v >> 2) & 0x33);
    return (v + (v >> 4)) & 0x0f;
  }
  function hammingBits(a: Uint8Array, b: Uint8Array): number {
    let n = 0;
    const m = Math.min(a.length, b.length);
    for (let i = 0; i < m; i++) n += popcount(a[i] ^ b[i]);
    return n;
  }

  $effect(() => {
    const sp = $page.url.searchParams;
    const a = sp.get('a');
    const b = sp.get('b');
    const algo = sp.get('algorithm');
    const modality = sp.get('modality') ?? 'text';
    const sr = sp.get('sample_rate');
    if (!a || !b || !algo) {
      err = 'diff viewer requires ?a, ?b, and ?algorithm in the URL';
      loading = false;
      return;
    }
    void load(a, b, algo, modality, sr);
  });

  async function load(a: string, b: string, algo: string, modality: string, sr: string | null) {
    loading = true;
    err = null;
    try {
      const [r1, r2] = await Promise.all([
        fingerprint(a, algo, modality, sr),
        fingerprint(b, algo, modality, sr),
      ]);
      algorithm = r1.algorithm;
      bytesA = hexToBytes(r1.hex);
      bytesB = hexToBytes(r2.hex);
    } catch (e) {
      err = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  async function fingerprint(
    inputId: string,
    algo: string,
    modality: string,
    sr: string | null,
  ): Promise<{ hex: string; algorithm: string }> {
    const qs = new URLSearchParams();
    qs.set('algorithm', algo);
    qs.set('input_id', inputId);
    if (modality === 'audio' && sr) qs.set('sample_rate', sr);
    const headers: Record<string, string> = {};
    if (modality === 'image') headers['content-type'] = 'image/png';
    else if (modality === 'audio') headers['content-type'] = 'audio/x-raw';
    else headers['content-type'] = 'text/plain; charset=utf-8';
    const res = await apiFetch(`/api/fingerprint?${qs.toString()}`, {
      method: 'POST',
      headers,
      body: new Uint8Array(0),
    });
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    const body = (await res.json()) as { fingerprint_hex: string; algorithm: string };
    return { hex: body.fingerprint_hex, algorithm: body.algorithm };
  }

  let hamming = $derived(bytesA && bytesB ? hammingBits(bytesA, bytesB) : 0);
  let bitsTotal = $derived(bytesA && bytesB ? Math.min(bytesA.length, bytesB.length) * 8 : 0);
  let matchPct = $derived(bitsTotal === 0 ? 0 : (1 - hamming / bitsTotal) * 100);
</script>

<div class="dv-viewer">
  {#if loading}
    <div class="dv-state">loading both inputs…</div>
  {:else if err}
    <div class="dv-state error"><strong>error:</strong> <pre>{err}</pre></div>
  {:else if bytesA && bytesB}
    <div class="dv-meta-bar">
      <span><strong>algorithm</strong> {algorithm}</span>
      <span><strong>bytes</strong> {bytesA.length} vs {bytesB.length}</span>
      <span><strong>hamming</strong> {hamming} / {bitsTotal} bits</span>
      <span><strong>match</strong> {matchPct.toFixed(2)}%</span>
    </div>
    <div class="dv-stage">
      {#if hasAlgorithmView(algorithm, bytesA.length) && bytesA.length === bytesB.length}
        <AlgorithmView algorithm={algorithm} bytes={bytesA} diffAgainst={bytesB} />
      {/if}
      <div class="dv-bitdiff">
        <div class="dv-bitdiff-label">bit-level XOR · A ⊕ B</div>
        <BitDiffStrip a={bytesA} b={bytesB} maxBytes={Math.min(bytesA.length, 256)} />
      </div>
    </div>
  {/if}
</div>

<style>
  .dv-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .dv-meta-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-2);
    background: rgba(255, 255, 255, 0.18);
  }
  .dv-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .dv-stage {
    flex: 1;
    min-height: 0;
    padding: 28px 32px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    overflow: auto;
  }
  .dv-bitdiff {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .dv-bitdiff-label {
    font-family: var(--mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .dv-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .dv-state.error {
    color: oklch(0.5 0.16 25);
  }
  .dv-state pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
  }
</style>
