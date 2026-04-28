<svelte:head>
  <title>Privacy — UCFP</title>
</svelte:head>

<div class="section-label">Legal</div>
<h1 class="legal-h">Privacy.</h1>

<div class="legal-body">
  <h2>What we store</h2>
  <ul>
    <li>Your email address and an argon2id hash of your password.</li>
    <li>Session records (a sha256 of the cookie token, expiry, IP, user agent) for the lifetime of each session.</li>
    <li>API keys you create — by sha256 hash, never the plaintext.</li>
    <li>Per-call usage events: timestamp, modality, byte count, HTTP status, latency. No request body content.</li>
  </ul>

  <h2>What we do NOT store</h2>
  <ul>
    <li>The plaintext content you fingerprint. UCFP is one-way by construction; the fingerprint cannot be reversed to bytes.</li>
    <li>Files you upload via the demo, unless you explicitly opt in with <code>?store=1</code> on an authenticated call.</li>
    <li>Any third-party tracking pixels or advertising identifiers.</li>
  </ul>

  <h2>Where we store it</h2>
  <p>
    Cloudflare D1 (SQLite at the edge) for relational data. Cloudflare KV for session and rate-limit caches.
    Cloudflare Workers Analytics Engine for aggregated usage metrics. Nothing is replicated to a third-party processor.
  </p>

  <h2>Retention</h2>
  <ul>
    <li>Sessions auto-expire after 30 days of inactivity.</li>
    <li>Usage events older than 90 days are pruned by a scheduled job.</li>
    <li>Account deletion (email <a href="mailto:privacy@ucfp.dev">privacy@ucfp.dev</a>) removes all rows referencing your user_id within 7 days.</li>
  </ul>

  <h2>Right to be forgotten</h2>
  <p>
    Delete the underlying record from your storage and the fingerprint stops resolving. We never see your raw bytes
    on the open-source or self-host tier. On the cloud tier, raw bytes are received only long enough to compute the
    fingerprint; the request body is not persisted unless you opt in.
  </p>

  <h2>Contact</h2>
  <p>
    Questions: <a href="mailto:privacy@ucfp.dev">privacy@ucfp.dev</a>. Effective: <time datetime="2026-04-28">2026-04-28</time>.
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
