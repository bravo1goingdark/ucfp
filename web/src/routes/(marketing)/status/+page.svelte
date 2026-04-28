<script lang="ts">
  import type { PageData } from './$types';
  let { data }: { data: PageData } = $props();

  const status = $derived(
    !data.configured ? 'unknown' : data.reachable ? 'operational' : 'degraded'
  );
  const dotColor = $derived(
    status === 'operational' ? 'oklch(0.66 0.15 145)' :
    status === 'degraded' ? 'oklch(0.62 0.18 30)' :
    'var(--muted)'
  );
</script>

<svelte:head>
  <title>Status — UCFP</title>
</svelte:head>

<div class="section-label">11 · Status</div>
<h1 class="status-title">
  {#if status === 'operational'}
    All systems <span class="it">operational.</span>
  {:else if status === 'degraded'}
    Investigating <span class="it">degradation.</span>
  {:else}
    Backend not <span class="it">configured.</span>
  {/if}
</h1>

<div class="status-grid">
  <div class="status-card">
    <span class="dot" style="background: {dotColor}"></span>
    <div>
      <div class="k">UCFP API</div>
      <div class="v">{status === 'operational' ? 'Reachable' : status === 'degraded' ? 'Unreachable' : 'No upstream URL set'}</div>
    </div>
  </div>
  <div class="status-card">
    <div class="k">Edge latency</div>
    <div class="v">{data.latencyMs !== null ? `${data.latencyMs} ms` : '—'}</div>
  </div>
  <div class="status-card">
    <div class="k">Edge POP</div>
    <div class="v">{data.colo ?? '—'}</div>
  </div>
  <div class="status-card">
    <div class="k">Checked at</div>
    <div class="v">{new Date(data.checkedAt).toISOString().slice(0, 19).replace('T', ' ')} UTC</div>
  </div>
</div>

<p class="footnote">
  Live check, not historical. Refresh the page for a fresh probe — the
  latency above is the edge-to-origin round-trip from this Cloudflare POP
  at the moment the page was rendered.
</p>

<style>
  .status-title {
    font-family: var(--sans);
    font-weight: 400;
    font-size: clamp(40px, 5vw, 64px);
    letter-spacing: -0.03em;
    line-height: 1.05;
    margin: 0 0 36px;
  }
  .status-title .it {
    font-family: var(--serif);
    font-style: italic;
  }
  .status-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0;
    border: 1px solid var(--line-strong);
    box-shadow: var(--paper-shadow);
  }
  .status-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 24px;
    border-right: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.25);
  }
  .status-card:last-child { border-right: 0; }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .k {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .v {
    font-family: var(--mono);
    font-size: 14px;
    color: var(--ink);
    margin-top: 4px;
  }
  .footnote {
    margin-top: 32px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    max-width: 56ch;
    line-height: 1.6;
  }
  @media (max-width: 800px) {
    .status-grid { grid-template-columns: 1fr 1fr; }
    .status-card { border-right: 0; border-bottom: 1px solid var(--line); }
  }
</style>
