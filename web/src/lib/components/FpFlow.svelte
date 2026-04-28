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
  // Each entry is { steps: string[] } for linear pipelines, or
  // { steps, branches } for parallel branches (image multi).

  interface LinearPipeline {
    kind: 'linear';
    steps: string[];
  }
  interface BranchPipeline {
    kind: 'branch';
    prefix: string[];   // nodes before fork
    branches: string[][]; // parallel branches
    suffix: string[];   // nodes after merge
  }
  type Pipeline = LinearPipeline | BranchPipeline;

  const PIPELINES: Record<string, Pipeline> = {
    // Text
    minhash:      { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','MinHash','Record'] },
    'simhash-tf': { kind:'linear', steps:['Input','Canonicalize','Tokenize','TF Weight','SimHash','Record'] },
    'simhash-idf':{ kind:'linear', steps:['Input','Canonicalize','Tokenize','IDF Weight','SimHash','Record'] },
    lsh:          { kind:'linear', steps:['Input','Canonicalize','Tokenize','Shingle','LSH Bands','Record'] },
    tlsh:         { kind:'linear', steps:['Input','Canonicalize','TLSH Hash','Record'] },
    // Image
    multi: {
      kind: 'branch',
      prefix: ['Input','Decode'],
      branches: [['PHash'], ['DHash'], ['AHash']],
      suffix: ['Merge','Record']
    },
    phash: { kind:'linear', steps:['Input','Decode','DCT','PHash','Record'] },
    dhash: { kind:'linear', steps:['Input','Decode','Gradient','DHash','Record'] },
    ahash: { kind:'linear', steps:['Input','Decode','Average','AHash','Record'] },
    // Text semantic
    'semantic-openai': { kind:'linear', steps:['Input','Canonicalize','OpenAI Embed API','Embedding','Record'] },
    'semantic-voyage': { kind:'linear', steps:['Input','Canonicalize','Voyage Embed API','Embedding','Record'] },
    'semantic-cohere': { kind:'linear', steps:['Input','Canonicalize','Cohere Embed API','Embedding','Record'] },
    'semantic-local':  { kind:'linear', steps:['Input','Canonicalize','ONNX Encoder','Embedding','Record'] },
    // Image semantic
    semantic: { kind:'linear', steps:['Input','Decode','ONNX CLIP','Embedding','Record'] },
    // Audio
    wang:    { kind:'linear', steps:['Input','Decode','Spectrogram','Constellation','Wang','Record'] },
    panako:  { kind:'linear', steps:['Input','Decode','Spectrogram','Panako','Record'] },
    haitsma: { kind:'linear', steps:['Input','Decode','Spectrogram','Haitsma','Record'] },
    neural:  { kind:'linear', steps:['Input','Decode','Log-Mel','ONNX Encoder','Embeddings','Record'] },
    watermark:{ kind:'linear', steps:['Input','Decode','Spectrogram','AudioSeal','WatermarkResult'] },
  };

  const NODE_H = 36;
  const NODE_W = 130;
  const X_GAP = 170;
  const Y_CENTER = 160;
  const Y_BRANCH_GAP = 90;

  function nodeStyle(isLast: boolean): string {
    const base = `background:var(--bg-2);border:1px solid var(--ink);border-radius:4px;font-family:var(--mono);font-size:11px;padding:6px 10px;width:${NODE_W}px;text-align:center;`;
    return isLast
      ? base + 'border-color:var(--accent-ink);color:var(--accent-ink);font-weight:600;'
      : base;
  }

  function buildLinear(steps: string[]): { nodes: Node[]; edges: Edge[] } {
    const nodes: Node[] = steps.map((label, i) => ({
      id: String(i),
      position: { x: i * X_GAP + 40, y: Y_CENTER - NODE_H / 2 },
      data: { label },
      style: nodeStyle(i === steps.length - 1),
      draggable: false,
      selectable: false,
      connectable: false,
    }));
    const edges: Edge[] = steps.slice(0, -1).map((_, i) => ({
      id: `e${i}-${i + 1}`,
      source: String(i),
      target: String(i + 1),
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
      nodes.push({
        id, position: { x: i * X_GAP + 40, y: Y_CENTER - NODE_H / 2 },
        data: { label }, style: nodeStyle(false), draggable: false, selectable: false, connectable: false
      });
      if (i > 0) edges.push({ id: `e${prefixIds[i-1]}-${id}`, source: prefixIds[i-1], target: id });
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
        nodes.push({
          id, position: { x: branchX + si * X_GAP, y: branchYs[bi] },
          data: { label }, style: nodeStyle(false), draggable: false, selectable: false, connectable: false
        });
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
      nodes.push({
        id, position: { x: suffixStartX + i * X_GAP, y: Y_CENTER - NODE_H / 2 },
        data: { label }, style: nodeStyle(isLast), draggable: false, selectable: false, connectable: false
      });
      if (i === 0) {
        branchEndIds.forEach(src => edges.push({ id: `e${src}-${id}`, source: src, target: id }));
      } else {
        edges.push({ id: `e${suffixIds[i-1]}-${id}`, source: suffixIds[i-1], target: id });
      }
    });

    return { nodes, edges };
  }

  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);

  $effect(() => {
    const key = algorithm;
    const pipeline = PIPELINES[key] ?? PIPELINES['minhash'];
    const built = pipeline.kind === 'branch'
      ? buildBranch(pipeline)
      : buildLinear(pipeline.steps);
    nodes = built.nodes;
    edges = built.edges;
  });
</script>

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
  >
    <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
  </SvelteFlow>
</div>

<style>
  .flow-wrap {
    width: 100%;
    height: 300px;
    background: var(--bg-2);
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--ink);
  }

  .flow-wrap :global(.svelte-flow) {
    background: transparent;
  }

  .flow-wrap :global(.svelte-flow__edge-path) {
    stroke: var(--ink-2);
    stroke-width: 1.5;
  }

  .flow-wrap :global(.svelte-flow__attribution) {
    background: transparent;
    color: var(--ink-2);
    font-size: 9px;
  }
</style>
