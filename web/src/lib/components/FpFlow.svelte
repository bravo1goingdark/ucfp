<script lang="ts">
  let {
    modality,
    algorithm
  }: {
    modality: 'text' | 'image' | 'audio';
    algorithm: string;
  } = $props();

  interface LinearPipeline { kind: 'linear'; steps: string[]; }
  interface BranchPipeline { kind: 'branch'; prefix: string[]; branches: string[][]; suffix: string[]; }
  type Pipeline = LinearPipeline | BranchPipeline;

  const PIPELINES: Record<string, Pipeline> = {
    minhash:      { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','MinHash','Record'] },
    'simhash-tf': { kind:'linear', steps:['Input','Canonicalize','Tokenize','TF Weight','SimHash','Record'] },
    'simhash-idf':{ kind:'linear', steps:['Input','Canonicalize','Tokenize','IDF Weight','SimHash','Record'] },
    lsh:          { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','LSH Bands','Record'] },
    tlsh:         { kind:'linear', steps:['Input','Canonicalize','TLSH Hash','Record'] },
    multi:        { kind:'branch', prefix:['Input','Decode'], branches:[['PHash'],['DHash'],['AHash']], suffix:['Merge','Record'] },
    phash:        { kind:'linear', steps:['Input','Decode','DCT','PHash','Record'] },
    dhash:        { kind:'linear', steps:['Input','Decode','Gradient','DHash','Record'] },
    ahash:        { kind:'linear', steps:['Input','Decode','Average','AHash','Record'] },
    'semantic-openai': { kind:'linear', steps:['Input','Canonicalize','OpenAI Embed','Embedding','Record'] },
    'semantic-voyage': { kind:'linear', steps:['Input','Canonicalize','Voyage Embed','Embedding','Record'] },
    'semantic-cohere': { kind:'linear', steps:['Input','Canonicalize','Cohere Embed','Embedding','Record'] },
    'semantic-local':  { kind:'linear', steps:['Input','Canonicalize','ONNX Encoder','Embedding','Record'] },
    semantic:    { kind:'linear', steps:['Input','Decode','ONNX CLIP','Embedding','Record'] },
    wang:        { kind:'linear', steps:['Input','Decode','Spectrogram','Peaks','Wang Hash','Record'] },
    panako:      { kind:'linear', steps:['Input','Decode','Spectrogram','Panako','Record'] },
    haitsma:     { kind:'linear', steps:['Input','Decode','Spectrogram','Haitsma','Record'] },
    neural:      { kind:'linear', steps:['Input','Decode','Log-Mel','ONNX Encoder','Embedding','Record'] },
    watermark:   { kind:'linear', steps:['Input','Decode','Spectrogram','AudioSeal','WatermarkResult'] },
  };

  const STEP_DESCS: Record<string, string> = {
    'Input':         'Raw bytes received from the client over HTTP.',
    'Canonicalize':  'Unicode normalization (NFKC), case folding, Bidi-control stripping.',
    'Tokenize':      'Split text into words or grapheme clusters (UAX #29).',
    'Shingle':       'Slide a k-width window over tokens to produce overlapping k-grams.',
    'MinHash':       'H independent hash functions over the shingle set; keep the minimum per function.',
    'TF Weight':     'Weight each token by term frequency before hashing.',
    'IDF Weight':    'Downweight common tokens via inverse document frequency.',
    'SimHash':       'Sum weighted binary hash vectors, threshold each bit at zero.',
    'LSH Bands':     'Partition MinHash into b bands of r rows; hash each band.',
    'TLSH Hash':     'Byte histogram → quartile thresholds → 48-byte hex digest.',
    'Decode':        'Decompress raw bytes into pixels or PCM samples.',
    'DCT':           'Discrete Cosine Transform on 8×8 pixel thumbnail.',
    'PHash':         'Top-left 8×8 DCT coefficients vs mean → 64 bits.',
    'Gradient':      'Horizontal pixel-pair intensity differences.',
    'DHash':         'Gradient sign bits → 64 bits.',
    'Average':       'Downsample to 8×8, compare each pixel to mean.',
    'AHash':         'Threshold 64 pixels against their mean.',
    'Merge':         'Concatenate PHash + DHash + AHash into one bundle.',
    'Spectrogram':   'STFT → time-frequency magnitude matrix.',
    'Peaks':         'Local amplitude peaks in the spectrogram.',
    'Wang Hash':     'Hash peak pairs into (f₁, f₂, Δt) landmark triplets.',
    'Panako':        'Three-point frequency constellation hashes.',
    'Haitsma':       'Sub-band energy ratio hashing at 5 kHz.',
    'Log-Mel':       'Mel-scale filterbank + log → neural model input.',
    'ONNX Encoder':  'Forward-pass through a local ONNX model.',
    'ONNX CLIP':     'CLIP vision encoder → 512-d semantic embedding.',
    'OpenAI Embed':  'POST to OpenAI Embeddings API → dense vector.',
    'Voyage Embed':  'POST to Voyage AI Embeddings API → dense vector.',
    'Cohere Embed':  'POST to Cohere Embed API → multilingual vector.',
    'Embedding':     'Dense float32 vector indexed in HNSW for ANN search.',
    'AudioSeal':     'Neural watermark detector → per-frame scores.',
    'WatermarkResult':'Binary detected/not-detected + confidence.',
    'Record':        'Serialise fingerprint + embedding into redb store.',
  };

  const STAGE_INPUT = new Set(['Input']);
  const STAGE_PREP  = new Set(['Canonicalize','Tokenize','Decode']);
  const STAGE_STORE = new Set(['Record']);
  const STAGE_EMB   = new Set(['Embedding','WatermarkResult']);

  const NODE_W = 120;
  const NODE_H = 32;
  const X_GAP = 155;
  const Y_CENTER = 140;
  const Y_BRANCH_GAP = 70;

  function nodeColor(label: string, isLast: boolean): { bg: string; border: string; text: string } {
    if (isLast || STAGE_STORE.has(label)) return { bg: '#d4edda', border: '#28a745', text: '#155724' };
    if (STAGE_INPUT.has(label)) return { bg: '#cce5ff', border: '#2563eb', text: '#1e3a8a' };
    if (STAGE_PREP.has(label)) return { bg: '#d1ecf1', border: '#17a2b8', text: '#0c5460' };
    if (STAGE_EMB.has(label)) return { bg: '#e2d9f3', border: '#7c3aed', text: '#4c1d95' };
    return { bg: '#fff3cd', border: '#d97706', text: '#78350f' };
  }

  interface RenderedNode { x: number; y: number; label: string; isLast: boolean; }
  interface RenderedEdge { x1: number; y1: number; x2: number; y2: number; }

  const layout = $derived.by(() => {
    const pipeline = PIPELINES[algorithm] ?? PIPELINES['minhash'];
    const nodes: RenderedNode[] = [];
    const edges: RenderedEdge[] = [];

    if (pipeline.kind === 'linear') {
      pipeline.steps.forEach((label, i) => {
        nodes.push({ x: i * X_GAP + 20, y: Y_CENTER - NODE_H / 2, label, isLast: i === pipeline.steps.length - 1 });
        if (i > 0) {
          edges.push({
            x1: (i - 1) * X_GAP + 20 + NODE_W,
            y1: Y_CENTER,
            x2: i * X_GAP + 20,
            y2: Y_CENTER
          });
        }
      });
    } else {
      // prefix
      pipeline.prefix.forEach((label, i) => {
        nodes.push({ x: i * X_GAP + 20, y: Y_CENTER - NODE_H / 2, label, isLast: false });
        if (i > 0) {
          edges.push({ x1: (i-1)*X_GAP+20+NODE_W, y1: Y_CENTER, x2: i*X_GAP+20, y2: Y_CENTER });
        }
      });
      const lastPrefixX = (pipeline.prefix.length - 1) * X_GAP + 20;
      const branchStartX = pipeline.prefix.length * X_GAP + 20;
      // branches
      const branchEndXs: { x: number; y: number }[] = [];
      pipeline.branches.forEach((branch, bi) => {
        const yOff = (bi - Math.floor(pipeline.branches.length / 2)) * Y_BRANCH_GAP;
        const branchY = Y_CENTER + yOff;
        branch.forEach((label, si) => {
          const nx = branchStartX + si * X_GAP;
          nodes.push({ x: nx, y: branchY - NODE_H / 2, label, isLast: false });
          if (si === 0) {
            edges.push({ x1: lastPrefixX + NODE_W, y1: Y_CENTER, x2: nx, y2: branchY });
          } else {
            edges.push({ x1: branchStartX + (si-1)*X_GAP + NODE_W, y1: branchY, x2: nx, y2: branchY });
          }
          if (si === branch.length - 1) branchEndXs.push({ x: nx + NODE_W, y: branchY });
        });
      });
      // suffix
      const suffixStartX = branchStartX + X_GAP;
      pipeline.suffix.forEach((label, i) => {
        const nx = suffixStartX + i * X_GAP;
        nodes.push({ x: nx, y: Y_CENTER - NODE_H / 2, label, isLast: i === pipeline.suffix.length - 1 });
        if (i === 0) {
          branchEndXs.forEach(be => { edges.push({ x1: be.x, y1: be.y, x2: nx, y2: Y_CENTER }); });
        } else {
          edges.push({ x1: suffixStartX + (i-1)*X_GAP + NODE_W, y1: Y_CENTER, x2: nx, y2: Y_CENTER });
        }
      });
    }
    const totalW = Math.max(...nodes.map(n => n.x + NODE_W)) + 20;
    const totalH = Y_CENTER * 2 + 20;
    return { nodes, edges, totalW, totalH };
  });

  let hoveredStep = $state<string | null>(null);
  const currentDesc = $derived(hoveredStep ? (STEP_DESCS[hoveredStep] ?? null) : null);
</script>

<div class="flow-container">
  <div class="flow-legend">
    <span class="legend-item" style="background:#cce5ff;border-color:#2563eb;color:#1e3a8a">Input</span>
    <span class="legend-item" style="background:#d1ecf1;border-color:#17a2b8;color:#0c5460">Pre-process</span>
    <span class="legend-item" style="background:#fff3cd;border-color:#d97706;color:#78350f">Transform</span>
    <span class="legend-item" style="background:#e2d9f3;border-color:#7c3aed;color:#4c1d95">Embedding</span>
    <span class="legend-item" style="background:#d4edda;border-color:#28a745;color:#155724">Storage</span>
  </div>
  <div class="flow-scroll">
    <svg
      viewBox="0 0 {layout.totalW} {layout.totalH}"
      width={layout.totalW}
      height={layout.totalH}
      class="flow-svg"
      role="img"
      aria-label="Algorithm pipeline: {algorithm}"
    >
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5"
          markerWidth="6" markerHeight="6" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#2A2A28" />
        </marker>
      </defs>
      <!-- Edges -->
      {#each layout.edges as e}
        <line
          x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
          stroke="#2A2A28" stroke-width="2" marker-end="url(#arrow)"
        />
      {/each}
      <!-- Nodes -->
      {#each layout.nodes as node}
        {@const c = nodeColor(node.label, node.isLast)}
        <g
          role="button"
          tabindex="0"
          onpointerenter={() => { hoveredStep = node.label; }}
          onpointerleave={() => { hoveredStep = null; }}
          onfocus={() => { hoveredStep = node.label; }}
          onblur={() => { hoveredStep = null; }}
        >
          <rect
            x={node.x} y={node.y} width={NODE_W} height={NODE_H} rx="4"
            fill={c.bg} stroke={c.border} stroke-width="1.5"
            class="node-rect"
            class:hovered={hoveredStep === node.label}
          />
          <text
            x={node.x + NODE_W / 2} y={node.y + NODE_H / 2 + 1}
            text-anchor="middle" dominant-baseline="middle"
            fill={c.text} font-size="10" font-family="var(--mono, monospace)"
          >{node.label}</text>
        </g>
      {/each}
    </svg>
  </div>
  <div class="step-desc" class:has-desc={!!currentDesc}>
    {#if currentDesc}
      <strong class="step-name">{hoveredStep}</strong>
      <span class="step-text">{currentDesc}</span>
    {:else}
      <span class="step-hint">Hover any step to see what it does</span>
    {/if}
  </div>
</div>

<style>
  .flow-container {
    display: flex; flex-direction: column;
    border: 1px solid var(--ink); border-radius: 6px; overflow: hidden;
  }
  .flow-legend {
    display: flex; gap: 0.5rem; padding: 6px 12px;
    background: var(--bg); border-bottom: 1px solid var(--ink);
    flex-wrap: wrap; align-items: center;
  }
  .legend-item {
    font-family: var(--mono); font-size: 10px;
    padding: 2px 8px; border-radius: 3px; border: 1px solid;
  }
  .flow-scroll {
    overflow-x: auto;
    background: var(--bg-2);
    padding: 12px;
  }
  .flow-svg { display: block; min-width: max-content; }
  .node-rect { cursor: pointer; transition: filter 0.12s; }
  .node-rect.hovered { filter: brightness(0.92); }
  .step-desc {
    min-height: 44px; padding: 8px 14px;
    background: var(--bg); border-top: 1px solid var(--ink);
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    font-family: var(--mono); font-size: 11.5px;
    transition: background 0.15s;
  }
  .step-desc.has-desc { background: color-mix(in oklch, var(--accent-ink) 6%, var(--bg)); }
  .step-name { font-weight: 700; color: var(--ink); white-space: nowrap; }
  .step-text { color: var(--ink-2); line-height: 1.5; }
  .step-hint { color: var(--muted); }
</style>
