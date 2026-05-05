<!--
  Viewer index — choose a visualization type. Reachable via deep-link
  paths /dashboard/playground/viewer/spectrogram?input_id=N etc. The
  index just describes what's available so a bookmark to /viewer alone
  doesn't 404.
-->
<script lang="ts">
  const links = [
    {
      href: '/dashboard/playground/viewer/spectrogram',
      title: 'Audio spectrogram',
      desc: 'Wang / Panako landmark constellation over the log-magnitude spectrogram.',
    },
    {
      href: '/dashboard/playground/viewer/heatmap',
      title: 'MinHash heatmap',
      desc: 'Per-slot heatmap for MinHash signatures. Diff two inputs to surface collisions.',
    },
    {
      href: '/dashboard/playground/viewer/wheel',
      title: 'SimHash wheel',
      desc: '64-bit polarity wheel for SimHash fingerprints with byte-boundary ticks.',
    },
    {
      href: '/dashboard/playground/viewer/embedding',
      title: 'Embedding bars',
      desc: 'Centred bars for each dimension of a dense embedding. Toolbar reports L2 + peak.',
    },
  ];
</script>

<section class="viewer-index">
  <h1>Viewer</h1>
  <p>
    Each viewer renders a single visualisation full-bleed for the screen.
    Open a viewer with an <code>?input_id=N</code> query string from the
    playground to inspect a cached input without re-uploading bytes.
  </p>
  <ul>
    {#each links as l}
      <li>
        <a href={l.href}>
          <span class="t">{l.title}</span>
          <span class="d">{l.desc}</span>
        </a>
      </li>
    {/each}
  </ul>
</section>

<style>
  .viewer-index {
    max-width: 720px;
    margin: 0 auto;
    padding: 60px 32px;
  }
  h1 {
    font-family: var(--sans);
    font-weight: 400;
    font-size: 36px;
    letter-spacing: -0.02em;
    margin: 0 0 12px;
  }
  p {
    color: var(--ink-2);
    font-size: 15px;
    margin: 0 0 28px;
    max-width: 60ch;
  }
  code {
    font-family: var(--mono);
    background: var(--bg-2);
    padding: 1px 6px;
    border: 1px solid var(--line);
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 0;
    border-top: 1px solid var(--line-strong);
    border-left: 1px solid var(--line-strong);
  }
  li {
    border-right: 1px solid var(--line-strong);
    border-bottom: 1px solid var(--line-strong);
  }
  a {
    display: block;
    padding: 22px 24px;
    text-decoration: none;
    color: inherit;
    transition: background 0.12s ease;
  }
  a:hover {
    background: rgba(20, 20, 20, 0.04);
  }
  .t {
    display: block;
    font-family: var(--sans);
    font-size: 17px;
    font-weight: 500;
    letter-spacing: -0.01em;
  }
  .d {
    display: block;
    margin-top: 6px;
    font-size: 13px;
    color: var(--ink-2);
  }
</style>
