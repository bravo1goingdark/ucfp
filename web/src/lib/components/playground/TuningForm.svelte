<!--
  TuningForm — schema-driven generic algorithm-tuning UI.

  Fetches `/api/algorithms` once on mount, picks the schema for the
  selected (modality, algorithm), and renders one labeled control per
  Tunable. Two-way binds an `opts` Record back to the parent so the
  request builder can splat it onto the URL.

  Designed as an *additive* panel: the playground page can keep its
  hardcoded primary controls and let TuningForm cover the long-tail
  knobs the manifest exposes (canonicalizer, Panako/Haitsma/Neural/
  Watermark configs, …).
-->
<script lang="ts">
  import { onMount } from 'svelte';

  type TunableKind = 'bool' | 'int' | 'float' | 'enum' | 'string' | 'secret';
  interface Tunable {
    name: string;
    label: string;
    help: string;
    kind: TunableKind;
    min?: number;
    max?: number;
    step?: number;
    enum_values?: string[];
    default_value?: unknown;
  }
  interface Preset { id: string; label: string; values: Record<string, unknown> }
  interface Algorithm {
    id: string;
    label: string;
    description: string;
    tunables: Tunable[];
    presets: Preset[];
  }
  interface ModalityCatalog { modality: string; algorithms: Algorithm[] }
  interface AlgorithmsResponse { modalities: ModalityCatalog[] }

  // Props
  let {
    modality,
    algorithm,
    opts = $bindable({} as Record<string, unknown>),
    /**
     * Tunables this list will *not* render — the playground page already
     * surfaces them via its primary controls. Defaults to the legacy
     * hardcoded set so the new panel only adds what was previously hidden.
     */
    skip = [
      'k', 'h', 'tokenizer', 'preprocess', 'model_id', 'api_key', 'sample_rate',
      'fan_out', 'peaks_per_sec', 'target_zone_t', 'target_zone_f', 'min_anchor_mag_db',
      'max_dimension', 'min_dimension', 'max_input_bytes',
    ] as string[],
  }: {
    modality: 'text' | 'image' | 'audio';
    algorithm: string;
    opts?: Record<string, unknown>;
    skip?: string[];
  } = $props();

  let manifest = $state<AlgorithmsResponse | null>(null);
  let loadErr = $state<string | null>(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      const res = await fetch('/api/algorithms');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      manifest = (await res.json()) as AlgorithmsResponse;
    } catch (e) {
      loadErr = (e as Error).message;
    } finally {
      loading = false;
    }
  });

  // Localstorage persistence: per (modality, algorithm) key.
  function storageKey(): string {
    return `ucfp.tuning.${modality}.${algorithm}`;
  }
  function loadPersisted() {
    try {
      const raw = localStorage.getItem(storageKey());
      if (!raw) return;
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      // Only restore fields not already set by the parent.
      for (const [k, v] of Object.entries(parsed)) {
        if (opts[k] === undefined && tunablesNotSkipped.find(t => t.name === k)) {
          opts[k] = v;
        }
      }
    } catch { /* ignore */ }
  }
  $effect(() => {
    if (typeof localStorage === 'undefined') return;
    try {
      const persistable: Record<string, unknown> = {};
      for (const t of tunablesNotSkipped) {
        if (opts[t.name] !== undefined && opts[t.name] !== '' && opts[t.name] !== null) {
          persistable[t.name] = opts[t.name];
        }
      }
      localStorage.setItem(storageKey(), JSON.stringify(persistable));
    } catch { /* ignore */ }
  });

  const algorithmEntry = $derived.by<Algorithm | null>(() => {
    if (!manifest) return null;
    const m = manifest.modalities.find(m => m.modality === modality);
    return m?.algorithms.find(a => a.id === algorithm) ?? null;
  });
  const tunablesNotSkipped = $derived.by<Tunable[]>(() => {
    if (!algorithmEntry) return [];
    return algorithmEntry.tunables.filter(t => !skip.includes(t.name));
  });

  // Restore persisted state when the algorithm switches.
  $effect(() => {
    void algorithm; void modality; void manifest;
    if (manifest) loadPersisted();
  });

  // Apply a preset by merging its values into opts.
  function applyPreset(p: Preset) {
    for (const [k, v] of Object.entries(p.values)) {
      if (skip.includes(k)) continue;
      opts[k] = v;
    }
  }
  function reset() {
    for (const t of tunablesNotSkipped) {
      delete opts[t.name];
    }
    try { localStorage.removeItem(storageKey()); } catch { /* ignore */ }
  }
</script>

{#if loading}
  <p class="muted small">Loading algorithm catalog…</p>
{:else if loadErr}
  <p class="muted small">Couldn't load algorithm manifest ({loadErr}). Tuning controls hidden.</p>
{:else if !algorithmEntry}
  <p class="muted small">No manifest entry for <code>{modality}/{algorithm}</code>.</p>
{:else if tunablesNotSkipped.length === 0}
  <p class="muted small">No additional tunables for this algorithm.</p>
{:else}
  <div class="tuning">
    {#if algorithmEntry.presets.length > 0}
      <div class="presets">
        <span class="presets-label">Presets:</span>
        {#each algorithmEntry.presets as p (p.id)}
          <button type="button" class="chip" onclick={() => applyPreset(p)}>{p.label}</button>
        {/each}
        <button type="button" class="chip ghost" onclick={reset}>Reset</button>
      </div>
    {/if}
    <div class="grid">
      {#each tunablesNotSkipped as t (t.name)}
        <label class="field">
          <span class="label">{t.label}</span>
          {#if t.kind === 'bool'}
            <select bind:value={opts[t.name]}>
              <option value={undefined}>(default)</option>
              <option value={true}>true</option>
              <option value={false}>false</option>
            </select>
          {:else if t.kind === 'enum'}
            <select bind:value={opts[t.name]}>
              <option value={undefined}>(default)</option>
              {#each t.enum_values ?? [] as v (v)}
                <option value={v}>{v}</option>
              {/each}
            </select>
          {:else if t.kind === 'int' || t.kind === 'float'}
            <input
              type="number"
              min={t.min}
              max={t.max}
              step={t.step ?? (t.kind === 'int' ? 1 : 'any')}
              placeholder="(default)"
              bind:value={opts[t.name]}
            />
          {:else if t.kind === 'secret'}
            <input type="password" autocomplete="off" placeholder="(default)" bind:value={opts[t.name]} />
          {:else}
            <input type="text" autocomplete="off" placeholder="(default)" bind:value={opts[t.name]} />
          {/if}
          {#if t.help}<span class="help">{t.help}</span>{/if}
        </label>
      {/each}
    </div>
  </div>
{/if}

<style>
  .tuning { display: grid; gap: 0.75rem; }
  .presets {
    display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center;
    padding: 0.4rem 0.6rem; background: var(--surface-2, rgba(255,255,255,0.03));
    border: 1px solid var(--border, rgba(255,255,255,0.08)); border-radius: 0.5rem;
  }
  .presets-label { font-size: 0.78rem; opacity: 0.7; margin-right: 0.4rem; }
  .chip {
    border: 1px solid var(--border, rgba(255,255,255,0.12));
    background: var(--surface, rgba(255,255,255,0.04));
    color: inherit; padding: 0.25rem 0.6rem; border-radius: 999px;
    font-size: 0.78rem; cursor: pointer;
  }
  .chip:hover { background: var(--surface-hover, rgba(255,255,255,0.08)); }
  .chip.ghost { opacity: 0.7; }
  .grid {
    display: grid; gap: 0.6rem 1rem;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  }
  .field { display: grid; gap: 0.25rem; }
  .label { font-size: 0.78rem; opacity: 0.85; }
  .help { font-size: 0.7rem; opacity: 0.6; line-height: 1.3; }
  input, select {
    background: var(--surface-2, rgba(255,255,255,0.04));
    color: inherit; border: 1px solid var(--border, rgba(255,255,255,0.12));
    padding: 0.35rem 0.5rem; border-radius: 0.4rem; font: inherit; width: 100%;
  }
  input:focus, select:focus {
    outline: 2px solid var(--accent, #6ad);
    outline-offset: 1px;
  }
  .muted { opacity: 0.65; }
  .small { font-size: 0.78rem; }
</style>
