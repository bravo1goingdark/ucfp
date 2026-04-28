<svelte:head>
  <title>Accessibility — UCFP</title>
</svelte:head>

<div class="section-label">Legal</div>
<h1 class="legal-h">Accessibility statement.</h1>

<div class="legal-body">
  <h2>Conformance target</h2>
  <p>
    UCFP targets <a href="https://www.w3.org/WAI/standards-guidelines/wcag/">WCAG 2.1 Level AA</a>.
    We track conformance via Lighthouse audits on every PR; the dashboard, docs, and landing page must score ≥ 95
    on the accessibility axis before merge.
  </p>

  <h2>What we've done</h2>
  <ul>
    <li>Visible focus rings on every interactive element via <code>:focus-visible</code>, never suppressed.</li>
    <li>Semantic HTML — <code>nav</code>, <code>main</code>, <code>article</code>, <code>aside</code> reflect document structure.</li>
    <li>Color contrast ≥ 4.5:1 across all three themes (paper / snow / ink). The accent color uses an <code>oklch</code> hue that meets contrast in both light and dark surfaces.</li>
    <li><code>prefers-reduced-motion</code> respected — animations collapse to ≤ 0.01ms.</li>
    <li><code>prefers-contrast: more</code> increases line and muted text strength.</li>
    <li>Keyboard navigation works on every interactive control (Tab, Enter, Space, Esc).</li>
    <li>Form fields have associated <code>label</code>s, descriptive error messages, and <code>aria-live</code> regions for async feedback.</li>
  </ul>

  <h2>Known limitations</h2>
  <ul>
    <li>The visual fingerprint grid in the demo is decorative and marked <code>aria-hidden</code>; the equivalent textual hash is the canonical output for screen readers.</li>
    <li>Charts on the dashboard include text summaries via <code>aria-label</code> but do not expose individual datapoints in a tabular form. A proper data-table fallback is on the roadmap.</li>
  </ul>

  <h2>Feedback</h2>
  <p>
    Found a barrier? Email <a href="mailto:accessibility@ucfp.dev">accessibility@ucfp.dev</a> and we'll respond within
    five business days. Critical issues (cannot complete a primary task) get a same-week fix.
  </p>

  <p style="margin-top: 36px;">
    Effective: <time datetime="2026-04-28">2026-04-28</time>.
  </p>
</div>

<style>
  .legal-h {
    font-family: var(--sans);
    font-weight: 400;
    font-size: clamp(48px, 6vw, 72px);
    letter-spacing: -0.03em;
    margin: 0 0 36px;
  }
  .legal-body {
    max-width: 70ch;
    color: var(--ink-2);
    font-size: 16px;
    line-height: 1.7;
  }
  .legal-body h2 {
    font-family: var(--sans);
    font-weight: 500;
    font-size: 22px;
    letter-spacing: -0.01em;
    margin: 36px 0 12px;
    color: var(--ink);
  }
  .legal-body ul { padding-left: 20px; }
  .legal-body li { margin: 6px 0; }
  .legal-body code {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--ink);
    background: var(--bg-2);
    padding: 1px 6px;
  }
</style>
