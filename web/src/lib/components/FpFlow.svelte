<script lang="ts">
  import { SvelteFlow, Background, BackgroundVariant } from '@xyflow/svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';

  let {
    modality,
    algorithm
  }: {
    modality: 'text' | 'image' | 'audio';
    algorithm: string;
  } = $props();

  // ── pipeline definitions ──────────────────────────────────────────────────
  interface LinearPipeline { kind: 'linear'; steps: string[]; }
  interface BranchPipeline {
    kind: 'branch';
    prefix: string[];
    branches: string[][];
    suffix: string[];
  }
  type Pipeline = LinearPipeline | BranchPipeline;

  const PIPELINES: Record<string, Pipeline> = {
    minhash:      { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','MinHash','Record'] },
    'simhash-tf': { kind:'linear', steps:['Input','Canonicalize','Tokenize','TF Weight','SimHash','Record'] },
    'simhash-idf':{ kind:'linear', steps:['Input','Canonicalize','Tokenize','IDF Weight','SimHash','Record'] },
    lsh:          { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','LSH Bands','Record'] },
    tlsh:         { kind:'linear', steps:['Input','Canonicalize','TLSH Hash','Record'] },
    multi: {
      kind: 'branch',
      prefix: ['Input','Decode'],
      branches: [['PHash'], ['DHash'], ['AHash']],
      suffix: ['Merge','Record']
    },
    phash: { kind:'linear', steps:['Input','Decode','DCT','PHash','Record'] },
    dhash: { kind:'linear', steps:['Input','Decode','Gradient','DHash','Record'] },
    ahash: { kind:'linear', steps:['Input','Decode','Average','AHash','Record'] },
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

  // ── per-step descriptions ────────────────────────────────────────────────
  const STEP_DESCS: Record<string, string> = {
    'Input':         'Raw bytes received from the client over HTTP — text, image, or PCM audio samples.',
    'Canonicalize':  'Unicode normalization (NFKC), case folding, and Bidi-control stripping to ensure the fingerprint is stable across text encoding variants.',
    'Tokenize':      'Split canonical text into words or grapheme clusters using UAX #29 Unicode rules.',
    'Shingle':       'Slide a k-width window over tokens to produce overlapping k-grams — the set of k-grams is the input to the hash function.',
    'MinHash':       'Apply H independent hash functions to the shingle set, keeping only the minimum per function. The resulting fixed-width signature approximates Jaccard similarity.',
    'TF Weight':     'Weight each token by how many times it appears (term frequency) before accumulating into the hash vector.',
    'IDF Weight':    'Downweight tokens that appear in many documents (inverse document frequency) to boost rare, discriminating terms.',
    'SimHash':       'Sum weighted binary hash vectors per token, then threshold each bit dimension at zero. Produces a compact fingerprint where Hamming distance ≈ cosine distance.',
    'LSH Bands':     'Partition the MinHash signature into b bands of r rows; hash each band separately. Records that match any band are candidate near-duplicates.',
    'TLSH Hash':     'Build a sliding-window byte histogram, compute quartile thresholds, encode as triplet values. Produces a 48-byte hex digest that supports edit-distance queries.',
    'Decode':        'Decompress or decode the raw bytes — JPEG/PNG/WebP for images; MP3/OGG/FLAC/WAV for audio — into raw pixel values or float32 PCM samples.',
    'DCT':           'Apply a Discrete Cosine Transform to an 8×8 pixel thumbnail, projecting the image into frequency space where low-frequency content dominates.',
    'PHash':         'Retain the top-left 8×8 DCT coefficients; compare each to the mean. Produces 64 bits stable under resize, JPEG re-encoding, and minor colour shift.',
    'Gradient':      'Compute horizontal pixel-pair intensity differences across a 9×8 grid to capture directional edge patterns.',
    'DHash':         'Encode gradient sign bits as 64 bits — stable under brightness and contrast changes, faster than PHash.',
    'Average':       'Downsample to 8×8 pixels, compare each pixel\'s brightness to the mean of all 64 pixels.',
    'AHash':         'Threshold the 64 average-sampled pixels against their mean. The fastest hash variant; best for graphics/cartoons.',
    'Merge':         'Concatenate the PHash, DHash, and AHash bit strings into a single multi-hash bundle for combined multi-factor comparison.',
    'Spectrogram':   'Short-Time Fourier Transform (STFT) converts raw PCM samples into a time-frequency magnitude matrix for pattern extraction.',
    'Peaks':         'Identify local amplitude peaks in the spectrogram — "constellation" points that are robust to noise and channel distortion.',
    'Wang Hash':     'Hash pairs of constellation peaks into (f₁, f₂, Δt) triplets — landmark hashes that survive pitch shift, speed change, and background noise.',
    'Panako':        'Extract three-point frequency constellations; hash their frequency ratios — invariant to playback speed changes up to ±20%.',
    'Haitsma':       'Compute 33 sub-band energy ratios per 11.6 ms frame and sign-encode each bit. Produces strings that survive codec re-encoding and noise.',
    'Log-Mel':       'Apply a mel-scale filterbank to spectrogram frequencies and take the log — mimics human auditory frequency perception as neural model input.',
    'ONNX Encoder':  'Run the input through a local ONNX model (BGE, E5, MiniLM for text; audio encoder for audio): tokenize or frame, forward-pass, mean-pool the output.',
    'ONNX CLIP':     'Run the image through a CLIP vision encoder (ONNX): resize to 224×224, normalise, forward-pass — produces a 512-d semantic embedding.',
    'OpenAI Embed':  'POST the canonical text to the OpenAI Embeddings API and receive a dense float32 vector (ada-002 or text-embedding-3-* models).',
    'Voyage Embed':  'POST the canonical text to the Voyage AI Embeddings API and receive a high-quality retrieval-optimised float32 vector.',
    'Cohere Embed':  'POST the canonical text to the Cohere Embed API; supports multilingual inputs for cross-language deduplication.',
    'Embedding':     'Dense float32 vector stored alongside the fingerprint bytes in redb and indexed in HNSW for approximate nearest-neighbour search.',
    'AudioSeal':     'Run the AudioSeal neural detector frame-by-frame; compute a per-frame watermark presence score from the spectrogram.',
    'WatermarkResult':'Aggregate per-frame detection scores into a binary decision (detected / not detected) plus a confidence percentage and optional decoded payload.',
    'Record':        'Serialise the fingerprint (and embedding if present) into the embedded redb store, then update the HNSW ANN index for vector search.',
  };

  // ── node colour by stage type ─────────────────────────────────────────────
  const STAGE_INPUT = new Set(['Input']);
  const STAGE_PREP  = new Set(['Canonicalize','Tokenize','Decode']);
  const STAGE_STORE = new Set(['Record']);
  const STAGE_EMB   = new Set(['Embedding','WatermarkResult']);

  const NODE_H = 38;
  const NODE_W = 130;
  const X_GAP  = 170;
  const Y_CENTER     = 180;
  const Y_BRANCH_GAP = 90;

  function nodeStyle(label: string, isLast: boolean): string {
    const base = `border:1.5px solid;border-radius:4px;font-family:var(--mono);font-size:11px;padding:7px 10px;width:${NODE_W}px;text-align:center;cursor:default;transition:box-shadow 0.15s;`;
    if (isLast || STAGE_STORE.has(label))
      return base + 'background:color-mix(in oklch,oklch(0.55 0.18 145) 14%,var(--bg-2));border-color:oklch(0.52 0.18 145);color:oklch(0.33 0.15 145);font-weight:600;';
    if (STAGE_INPUT.has(label))
      return base + 'background:color-mix(in oklch,oklch(0.55 0.18 240) 14%,var(--bg-2));border-color:oklch(0.52 0.18 240);color:oklch(0.33 0.15 240);';
    if (STAGE_PREP.has(label))
      return base + 'background:color-mix(in oklch,oklch(0.55 0.16 200) 14%,var(--bg-2));border-color:oklch(0.52 0.16 200);color:oklch(0.33 0.14 200);';
    if (STAGE_EMB.has(label))
      return base + 'background:color-mix(in oklch,oklch(0.55 0.18 300) 14%,var(--bg-2));border-color:oklch(0.52 0.18 300);color:oklch(0.33 0.15 300);';
    // transform / hash step — amber
    return base + 'background:color-mix(in oklch,oklch(0.55 0.18 60) 14%,var(--bg-2));border-color:oklch(0.52 0.18 60);color:oklch(0.33 0.16 60);';
  }

  function buildLinear(steps: string[]): { nodes: Node[]; edges: Edge[] } {
    const nodes: Node[] = steps.map((label, i) => ({
      id: String(i),
      position: { x: i * X_GAP + 40, y: Y_CENTER - NODE_H / 2 },
      data: { label },
      style: nodeStyle(label, i === steps.length - 1),
      draggable: false, selectable: false, connectable: false,
    }));
    const edges: Edge[] = steps.slice(0, -1).map((_, i) => ({
      id: `e${i}-${i + 1}`, source: String(i), target: String(i + 1),
    }));
    return { nodes, edges };
  }

  function buildBranch(p: BranchPipeline): { nodes: Node[]; edges: Edge[] } {
    const nodes: Node[] = [];
    const edges: Edge[] = [];
    let idCounter = 0;

    const prefixIds: string[] = [];
    p.prefix.forEach((label, i) => {
      const id = String(idCounter++);
      prefixIds.push(id);
      nodes.push({ id, position: { x: i * X_GAP + 40, y: Y_CENTER - NODE_H / 2 },
        data: { label }, style: nodeStyle(label, false), draggable: false, selectable: false, connectable: false });
      if (i > 0) edges.push({ id: `e${prefixIds[i - 1]}-${id}`, source: prefixIds[i - 1], target: id });
    });

    const lastPrefixId = prefixIds[prefixIds.length - 1];
    const branchX = p.prefix.length * X_GAP + 40;
    const branchYs = p.branches.map((_, i) =>
      Y_CENTER - NODE_H / 2 + (i - Math.floor(p.branches.length / 2)) * Y_BRANCH_GAP
    );

    const branchEndIds: string[] = [];
    p.branches.forEach((branch, bi) => {
      let prevId = lastPrefixId;
      branch.forEach((label, si) => {
        const id = String(idCounter++);
        if (si === branch.length - 1) branchEndIds.push(id);
        nodes.push({ id, position: { x: branchX + si * X_GAP, y: branchYs[bi] },
          data: { label }, style: nodeStyle(label, false), draggable: false, selectable: false, connectable: false });
        edges.push({ id: `e${prevId}-${id}`, source: prevId, target: id });
        prevId = id;
      });
    });

    const suffixStartX = branchX + X_GAP;
    const suffixIds: string[] = [];
    p.suffix.forEach((label, i) => {
      const isLast = i === p.suffix.length - 1;
      const id = String(idCounter++);
      suffixIds.push(id);
      nodes.push({ id, position: { x: suffixStartX + i * X_GAP, y: Y_CENTER - NODE_H / 2 },
        data: { label }, style: nodeStyle(label, isLast), draggable: false, selectable: false, connectable: false });
      if (i === 0) branchEndIds.forEach(src => edges.push({ id: `e${src}-${id}`, source: src, target: id }));
      else edges.push({ id: `e${suffixIds[i - 1]}-${id}`, source: suffixIds[i - 1], target: id });
    });
    return { nodes, edges };
  }

  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  let hoveredStep = $state<string | null>(null);

  $effect(() => {
    const pipeline = PIPELINES[algorithm] ?? PIPELINES['minhash'];
    const built = pipeline.kind === 'branch' ? buildBranch(pipeline) : buildLinear(pipeline.steps);
    nodes = built.nodes;
    edges = built.edges;
    hoveredStep = null;
  });

  function handleNodeEnter({ node }: { node: Node; event: PointerEvent }) {
    hoveredStep = String(node?.data?.label ?? '') || null;
  }
  function handleNodeLeave() { hoveredStep = null; }

  const currentDesc = $derived(hoveredStep ? (STEP_DESCS[hoveredStep] ?? null) : null);
</script>

<div class="flow-container">
  <div class="flow-legend">
    <span class="legend-item input">Input</span>
    <span class="legend-item prep">Pre-process</span>
    <span class="legend-item hash">Transform</span>
    <span class="legend-item emb">Embedding</span>
    <span class="legend-item store">Storage</span>
  </div>
  <div class="flow-wrap">
    <SvelteFlow
      bind:nodes
      bind:edges
      fitView
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable={false}
      panOnDrag={false}
      zoomOnScroll={false}
      zoomOnPinch={false}
      panOnScroll={false}
      deleteKey={null}
      attributionPosition="bottom-right"
      onnodepointerenter={handleNodeEnter}
      onnodepointerleave={handleNodeLeave}
    >
      <Background variant={BackgroundVariant.Dots} gap={18} size={1} />
    </SvelteFlow>
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
    display: flex; flex-direction: column; gap: 0;
    border: 1px solid var(--ink); border-radius: 6px; overflow: hidden;
  }

  .flow-legend {
    display: flex; gap: 0.6rem; padding: 6px 12px;
    background: var(--bg); border-bottom: 1px solid var(--ink);
    flex-wrap: wrap; align-items: center;
  }
  .legend-item {
    font-family: var(--mono); font-size: 10px;
    padding: 2px 8px; border-radius: 3px; border: 1px solid;
  }
  .legend-item.input { background:color-mix(in oklch,oklch(0.55 0.18 240) 14%,var(--bg-2)); border-color:oklch(0.52 0.18 240); color:oklch(0.33 0.15 240); }
  .legend-item.prep  { background:color-mix(in oklch,oklch(0.55 0.16 200) 14%,var(--bg-2)); border-color:oklch(0.52 0.16 200); color:oklch(0.33 0.14 200); }
  .legend-item.hash  { background:color-mix(in oklch,oklch(0.55 0.18 60) 14%,var(--bg-2));  border-color:oklch(0.52 0.18 60);  color:oklch(0.33 0.16 60);  }
  .legend-item.emb   { background:color-mix(in oklch,oklch(0.55 0.18 300) 14%,var(--bg-2)); border-color:oklch(0.52 0.18 300); color:oklch(0.33 0.15 300); }
  .legend-item.store { background:color-mix(in oklch,oklch(0.55 0.18 145) 14%,var(--bg-2)); border-color:oklch(0.52 0.18 145); color:oklch(0.33 0.15 145); }

  .flow-wrap {
    width: 100%; height: 320px;
    background: var(--bg-2);
    /* svelte-flow renders fixed-width nodes (130 px) with 170 px gaps,
       so the diagram itself is wider than a phone viewport. Allow the
       wrapper to scroll horizontally instead of clipping nodes off. */
    overflow-x: auto;
  }
  @media (max-width: 520px) {
    .flow-wrap { height: 360px; }
  }
  .flow-wrap :global(.svelte-flow) { background: transparent; }
  .flow-wrap :global(.svelte-flow__edge-path) { stroke: var(--ink-2); stroke-width: 1.5; }
  .flow-wrap :global(.svelte-flow__attribution) {
    background: transparent; color: var(--ink-2); font-size: 9px;
  }

  .step-desc {
    min-height: 52px; padding: 8px 14px;
    background: var(--bg); border-top: 1px solid var(--ink);
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    font-family: var(--mono); font-size: 11.5px;
    transition: background 0.15s;
  }
  .step-desc.has-desc { background: color-mix(in oklch, var(--accent-ink) 6%, var(--bg)); }
  .step-name { font-weight: 700; color: var(--ink); white-space: nowrap; }
  .step-text { color: var(--ink-2); line-height: 1.5; }
  .step-hint { color: var(--ink-2); opacity: 0.6; font-style: italic; }
</style>
