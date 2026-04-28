<svelte:head>
  <title>Terms — UCFP</title>
</svelte:head>

<div class="section-label">Legal</div>
<h1 class="legal-h">Terms of service.</h1>

<div class="legal-body">
  <h2>Open-source license</h2>
  <p>
    The UCFP runtime is released under the <a href="https://www.apache.org/licenses/LICENSE-2.0">Apache License 2.0</a>.
    The protocol specification (UCFP-1) is freely implementable. There is no patent-encumbered algorithm in either.
  </p>

  <h2>Acceptable use of the hosted API</h2>
  <ul>
    <li>No fingerprinting of content you do not have a lawful right to process.</li>
    <li>No attempts to abuse the hosted API as a content storage service — UCFP is one-way; the bytes are not retrievable.</li>
    <li>No automated mass-creation of accounts to circumvent rate limits.</li>
    <li>Reasonable security research is welcome; report findings to <a href="mailto:security@ucfp.dev">security@ucfp.dev</a>.</li>
  </ul>

  <h2>Service availability</h2>
  <p>
    The free tier is offered as-is, without an SLA. Best-effort uptime is published on the
    <a href="/status">status page</a>. Quotas and rate limits may change with reasonable notice.
  </p>

  <h2>Termination</h2>
  <p>
    You may close your account at any time. We reserve the right to suspend accounts that violate the acceptable-use
    rules above; we'll notify you before any irreversible action when contact information is on file.
  </p>

  <h2>Liability</h2>
  <p>
    The hosted service is provided without warranties of any kind. To the maximum extent permitted by law,
    the operators of UCFP are not liable for any indirect, incidental, or consequential damages arising from your use.
  </p>

  <h2>Governing law</h2>
  <p>
    These terms are governed by the laws of the jurisdiction where the operator is registered. A copy of the current
    operator entity is published on the <a href="/status">status page</a>.
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
</style>
