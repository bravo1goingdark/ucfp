<script lang="ts">
  const tiers = [
    {
      name: 'OPEN',
      amount: '$0',
      unit: '/ forever',
      desc: 'The full Apache-2.0 SDK. Self-host the runtime; own your fingerprints.',
      bullets: [
        'Every modality + algorithm',
        'WASM, Rust, Python, TS, Go bindings',
        'Self-hosted server (axum + redb)',
        'No telemetry, no rate limits',
        'GitHub issues only'
      ],
      cta: 'Clone the repo',
      featured: false
    },
    {
      name: 'CLOUD',
      amount: '$49',
      unit: '/ month',
      desc: 'Hosted UCFP API on Cloudflare. Scale-to-zero, regional, signed responses.',
      bullets: [
        '10M fingerprints / month included',
        '$0.000004 per additional call',
        'Managed Vectorize index',
        'API tokens + audit log',
        'Email support · 1-day SLA'
      ],
      cta: 'Start free trial',
      featured: true
    },
    {
      name: 'ENTERPRISE',
      amount: 'Talk',
      unit: 'to us',
      desc: 'BYOC, private regions, signed SBOMs, and a designated compliance contact.',
      bullets: [
        'Unlimited throughput',
        'On-prem or private VPC',
        'SOC 2 + HIPAA-ready',
        'SAML SSO + RBAC',
        'Solutions engineering hours'
      ],
      cta: 'Book a call',
      featured: false
    }
  ];
</script>

<section class="pricing" id="pricing">
  <div class="feat-head">
    <div>
      <div class="section-label">08 · Pricing</div>
      <h2 class="h">Open core. <span class="it">Honest cloud.</span></h2>
    </div>
    <p>
      The protocol is permissive Apache-2.0 forever. The hosted API is metered per call with a real
      free tier, no SaaS-shaped pricing tricks.
    </p>
  </div>

  <div class="price-grid">
    {#each tiers as t}
      <div class="tier" class:featured={t.featured}>
        <span class="name">{t.name}</span>
        <div class="price">
          <span class="amt">{t.amount}</span>
          <span class="unit">{t.unit}</span>
        </div>
        <p class="desc">{t.desc}</p>
        <ul>
          {#each t.bullets as b}
            <li>{b}</li>
          {/each}
        </ul>
        <a class="btn cta" class:alt={!t.featured} href="#cta">
          {t.cta} <span class="arrow">→</span>
        </a>
      </div>
    {/each}
  </div>
</section>
