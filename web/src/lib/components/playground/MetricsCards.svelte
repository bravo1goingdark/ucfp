<!--
  Five-cell metrics grid shown beneath the fingerprint visualization.
  Pure presentational — caller threads in the values.
-->
<script lang="ts">
  interface Props {
    algorithm: string;
    configHash: string;
    entropy: string;
    fpBytes: string;
    latencyMs: string;
  }
  let { algorithm, configHash, entropy, fpBytes, latencyMs }: Props = $props();
</script>

<div class="metrics-grid">
  <div class="metric-card"><span class="metric-k">Algorithm</span><span class="metric-v">{algorithm}</span></div>
  <div class="metric-card"><span class="metric-k">Config hash</span><span class="metric-v mono">{configHash}</span></div>
  <div class="metric-card"><span class="metric-k">Entropy</span><span class="metric-v">{entropy}</span></div>
  <div class="metric-card"><span class="metric-k">FP size</span><span class="metric-v">{fpBytes}</span></div>
  <div class="metric-card"><span class="metric-k">Latency</span><span class="metric-v">{latencyMs}</span></div>
</div>

<style>
  .metrics-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .metric-card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0.4rem 0.6rem;
    background: var(--bg);
    border-radius: 4px;
    border: 1px solid var(--ink);
  }
  .metric-card:last-child:nth-child(odd) { grid-column: 1 / -1; }
  .metric-k {
    font-family: var(--mono);
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-2);
  }
  .metric-v {
    font-family: var(--mono);
    font-size: 0.8rem;
    color: var(--ink);
    font-weight: 600;
  }
  .mono { word-break: break-all; }
</style>
