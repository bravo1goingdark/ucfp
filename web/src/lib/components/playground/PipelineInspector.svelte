<!--
  PipelineInspector — surfaces every intermediate stage of the
  fingerprinting pipeline so users can see what each step produced
  (raw → canonicalized → tokens → shingles → final hash).

  Text only for now. Image and audio stages return 501 from the proxy
  until the per-modality extractors land in src/server/handlers.rs;
  the component renders an explanatory placeholder for those.

  Triggered explicitly via the "Inspect pipeline" button — never on
  every keystroke or slider tick. Caches the last result client-side.
-->
<script lang="ts">
  type Props = {
    modality: 'text' | 'image' | 'audio';
    /** UTF-8 text body (text modality). */
    text: string;
    /** Cached input id for live-tune; sent instead of a body when present. */
    inputId?: number | null;
    /** Live tunables forwarded as query params (subset relevant to inspect). */
    opts?: Record<string, unknown>;
  };
  let { modality, text, inputId = null, opts = {} }: Props = $props();

  type TextStages = {
    algorithm: string;
    raw: string;
    canonicalized: string;
    tokens: string[];
    total_tokens: number;
    shingles: string[];
    total_shingles: number;
    fingerprint_hex: string;
    fingerprint_bytes: number;
    config_hash: number;
  };

  let result = $state<TextStages | null>(null);
  let loading = $state(false);
  let errMsg = $state<string | null>(null);
  let openStage = $state<string | null>('canonicalized');

  const INSPECT_OPT_KEYS = [
    'k','h','tokenizer','preprocess',
    'canon_normalization','canon_case_fold','canon_strip_bidi',
    'canon_strip_format','canon_apply_confusable',
  ];

  async function run(): Promise<void> {
    if (loading) return;
    if (modality !== 'text') {
      errMsg = `Pipeline inspect for ${modality} isn't implemented yet — text only.`;
      return;
    }
    errMsg = null;
    loading = true;
    try {
      const sp = new URLSearchParams({ modality });
      if (inputId != null) sp.set('input_id', String(inputId));
      for (const k of INSPECT_OPT_KEYS) {
        const v = opts[k];
        if (v == null || v === '') continue;
        sp.set(k, String(v));
      }
      const res = await fetch(`/api/pipeline/inspect?${sp.toString()}`, {
        method: 'POST',
        headers: { 'content-type': 'text/plain; charset=utf-8' },
        body: inputId != null ? '' : text,
      });
      if (!res.ok) {
        const detail = await res.text().catch(() => String(res.status));
        errMsg = `Inspect failed (${res.status}): ${detail.slice(0, 200)}`;
        return;
      }
      result = (await res.json()) as TextStages;
      openStage ??= 'canonicalized';
    } catch (e) {
      errMsg = `Inspect error: ${(e as Error).message}`;
    } finally {
      loading = false;
    }
  }

  // Highlight characters that differ between raw and canonicalized so
  // the user can see e.g. "é" → "é" (NFKC) or zero-widths being stripped.
  type Span = { text: string; changed: boolean };
  function diffSpans(raw: string, canon: string): Span[] {
    // Cheap two-pointer character walk — not a true diff, just "did this
    // codepoint change vs the corresponding position?". Good enough for
    // a glance; bails to a single-span when lengths differ enough.
    if (Math.abs(raw.length - canon.length) > Math.min(raw.length, canon.length) / 2) {
      return [{ text: canon, changed: false }];
    }
    const out: Span[] = [];
    let buf = '';
    let bufChanged = false;
    const n = Math.min(raw.length, canon.length);
    for (let i = 0; i < n; i++) {
      const same = raw[i] === canon[i];
      if (buf.length === 0) {
        buf = canon[i];
        bufChanged = !same;
      } else if (bufChanged === !same) {
        buf += canon[i];
      } else {
        out.push({ text: buf, changed: bufChanged });
        buf = canon[i];
        bufChanged = !same;
      }
    }
    if (buf.length > 0) out.push({ text: buf, changed: bufChanged });
    if (canon.length > n) out.push({ text: canon.slice(n), changed: true });
    return out;
  }

  function toggle(stage: string) {
    openStage = openStage === stage ? null : stage;
  }
</script>

<div class="inspector">
  <header class="inspector-head">
    <span class="inspector-title">Pipeline inspector</span>
    <button type="button" class="inspect-btn" onclick={run} disabled={loading}>
      {loading ? 'Inspecting…' : (result ? 'Re-inspect' : 'Inspect pipeline')}
    </button>
  </header>

  {#if errMsg}
    <p class="err" role="alert">{errMsg}</p>
  {/if}

  {#if result}
    {@const spans = diffSpans(result.raw, result.canonicalized)}
    {@const changedCount = spans.filter(s => s.changed).reduce((a, s) => a + s.text.length, 0)}

    <section class="stage" class:open={openStage === 'raw'}>
      <button type="button" class="stage-head" onclick={() => toggle('raw')}>
        <span class="step-num">1</span>
        <span class="stage-label">Raw input</span>
        <span class="stage-meta">{result.raw.length} chars</span>
      </button>
      {#if openStage === 'raw'}
        <pre class="stage-body mono">{result.raw}</pre>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'canonicalized'}>
      <button type="button" class="stage-head" onclick={() => toggle('canonicalized')}>
        <span class="step-num">2</span>
        <span class="stage-label">Canonicalized</span>
        <span class="stage-meta">
          {result.canonicalized.length} chars · {changedCount} changed
        </span>
      </button>
      {#if openStage === 'canonicalized'}
        <pre class="stage-body mono">{#each spans as s, i (i)}<span class:diff={s.changed}>{s.text}</span>{/each}</pre>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'tokens'}>
      <button type="button" class="stage-head" onclick={() => toggle('tokens')}>
        <span class="step-num">3</span>
        <span class="stage-label">Tokens</span>
        <span class="stage-meta">
          {result.tokens.length} of {result.total_tokens}
          {result.tokens.length < result.total_tokens ? '(truncated)' : ''}
        </span>
      </button>
      {#if openStage === 'tokens'}
        <div class="stage-body chips">
          {#each result.tokens as t, i (i)}<span class="chip mono">{t}</span>{/each}
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'shingles'}>
      <button type="button" class="stage-head" onclick={() => toggle('shingles')}>
        <span class="step-num">4</span>
        <span class="stage-label">k-shingles</span>
        <span class="stage-meta">
          {result.shingles.length} of {result.total_shingles}
          {result.shingles.length < result.total_shingles ? '(truncated)' : ''}
        </span>
      </button>
      {#if openStage === 'shingles'}
        <div class="stage-body chips">
          {#each result.shingles as s, i (i)}<span class="chip mono shingle">{s}</span>{/each}
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'fingerprint'}>
      <button type="button" class="stage-head" onclick={() => toggle('fingerprint')}>
        <span class="step-num">5</span>
        <span class="stage-label">Fingerprint</span>
        <span class="stage-meta">
          {result.algorithm} · {result.fingerprint_bytes} bytes
        </span>
      </button>
      {#if openStage === 'fingerprint'}
        <div class="stage-body">
          <div class="fp-meta mono">config_hash 0x{result.config_hash.toString(16)}</div>
          <pre class="fp-hex mono">{result.fingerprint_hex.slice(0, 256)}{result.fingerprint_hex.length > 256 ? '…' : ''}</pre>
        </div>
      {/if}
    </section>
  {:else if !loading && !errMsg}
    <p class="hint">
      Click <strong>Inspect pipeline</strong> to see each stage —
      raw → canonicalized → tokens → shingles → final hash.
      {#if modality !== 'text'}
        <br /><em>Pipeline inspect for {modality} isn't implemented yet.</em>
      {/if}
    </p>
  {/if}
</div>

<style>
  .inspector {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.75rem 0.85rem;
    background: var(--bg-2, rgba(255,255,255,0.03));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: 0.55rem;
  }
  .inspector-head {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.6rem;
  }
  .inspector-title {
    font-size: 0.78rem; opacity: 0.85;
    text-transform: uppercase; letter-spacing: 0.06em;
  }
  .inspect-btn {
    appearance: none; cursor: pointer;
    border: 1px solid var(--border, rgba(255,255,255,0.18));
    background: var(--surface, rgba(255,255,255,0.04));
    color: inherit;
    padding: 0.32rem 0.7rem; border-radius: 0.4rem;
    font: inherit; font-size: 0.8rem;
  }
  .inspect-btn:hover:not(:disabled) { background: var(--surface-hover, rgba(255,255,255,0.08)); }
  .inspect-btn:disabled { opacity: 0.55; cursor: progress; }
  .err {
    margin: 0; padding: 0.4rem 0.55rem; border-radius: 0.35rem;
    background: oklch(0.32 0.18 30 / 0.18);
    color: oklch(0.85 0.18 30);
    font-size: 0.78rem;
  }
  .hint {
    margin: 0.25rem 0 0;
    font-size: 0.78rem; opacity: 0.7;
    line-height: 1.4;
  }
  .stage {
    border: 1px solid var(--border, rgba(255,255,255,0.06));
    border-radius: 0.4rem;
    background: var(--bg, rgba(255,255,255,0.02));
    overflow: hidden;
  }
  .stage.open { background: var(--bg-3, rgba(255,255,255,0.045)); }
  .stage-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 0.6rem; align-items: center;
    width: 100%;
    appearance: none; border: 0; background: transparent; color: inherit;
    padding: 0.45rem 0.7rem; cursor: pointer;
    font: inherit; text-align: left;
  }
  .stage-head:hover { background: var(--surface-hover, rgba(255,255,255,0.04)); }
  .step-num {
    display: inline-flex; align-items: center; justify-content: center;
    width: 1.4rem; height: 1.4rem;
    font-family: var(--mono, monospace); font-size: 0.7rem;
    border-radius: 999px;
    background: var(--accent, oklch(0.55 0.18 240));
    color: var(--accent-ink, #fff);
  }
  .stage-label { font-size: 0.85rem; }
  .stage-meta {
    font-family: var(--mono, monospace); font-size: 0.7rem;
    opacity: 0.65;
  }
  .stage-body {
    padding: 0.6rem 0.7rem 0.7rem;
    border-top: 1px dashed var(--border, rgba(255,255,255,0.06));
  }
  .mono { font-family: var(--mono, monospace); font-size: 0.78rem; }
  pre.stage-body, pre.fp-hex {
    margin: 0; white-space: pre-wrap; word-break: break-word;
    max-height: 180px; overflow-y: auto;
  }
  pre.fp-hex {
    background: var(--bg, rgba(0,0,0,0.25));
    padding: 0.5rem 0.6rem; border-radius: 0.3rem;
    color: oklch(0.85 0.04 240);
  }
  .fp-meta { margin-bottom: 0.4rem; opacity: 0.7; }
  .diff {
    background: oklch(0.55 0.18 90 / 0.32);
    border-radius: 2px;
    padding: 0 1px;
  }
  .chips {
    display: flex; flex-wrap: wrap; gap: 0.25rem;
    max-height: 180px; overflow-y: auto;
  }
  .chip {
    display: inline-block;
    padding: 0.1rem 0.45rem;
    background: var(--bg, rgba(255,255,255,0.04));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: 0.4rem;
    font-size: 0.72rem;
  }
  .chip.shingle {
    background: oklch(0.5 0.12 240 / 0.16);
    border-color: oklch(0.5 0.12 240 / 0.3);
  }
</style>
