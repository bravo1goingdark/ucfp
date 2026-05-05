<!--
  Watermark detection result panel. Renders the detected/not-detected
  pill, confidence, latency, and (optional) payload hex.
-->
<script lang="ts">
  interface Props {
    detected: boolean;
    confidence: number;
    latencyMs: string;
    payload?: string | null;
  }
  let { detected, confidence, latencyMs, payload = null }: Props = $props();
</script>

<div class="pane-label">Watermark detection result</div>
<div class="wm-result">
  <span class="wm-pill" class:detected>
    {detected ? '✓ Watermark detected' : '✗ No watermark detected'}
  </span>
  <div class="metrics-grid">
    <div class="metric-card">
      <span class="metric-k">Confidence</span>
      <span class="metric-v">{(confidence * 100).toFixed(1)}%</span>
    </div>
    <div class="metric-card">
      <span class="metric-k">Latency</span>
      <span class="metric-v">{latencyMs}</span>
    </div>
    {#if payload}
      <div class="metric-card" style="grid-column:1/-1">
        <span class="metric-k">Payload</span>
        <span class="metric-v mono">{payload}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .pane-label {
    font-family: var(--mono);
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-2);
    margin-bottom: 0.5rem;
  }
  .wm-result { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
  .wm-pill {
    display: inline-flex;
    align-items: center;
    font-family: var(--mono);
    font-size: 0.82rem;
    font-weight: 600;
    padding: 7px 16px;
    border-radius: 20px;
    background: color-mix(in oklch, var(--ink) 6%, transparent);
    color: var(--ink-2);
    border: 1px solid var(--ink);
    align-self: flex-start;
  }
  .wm-pill.detected {
    background: color-mix(in oklch, oklch(0.58 0.18 145) 15%, transparent);
    color: oklch(0.38 0.15 145);
    border-color: oklch(0.58 0.18 145);
  }
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
