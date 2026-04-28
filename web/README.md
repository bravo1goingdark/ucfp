# ucfp-web

SvelteKit landing page for **Universal Content Fingerprinting**, deployed to
Cloudflare Pages. Ports the editorial design from `claude.ai/design` and
extends it into a full SaaS landing page (hero, demo, usage, capabilities,
inputs, how-it-works, use cases, integrations, pricing, testimonials, FAQ,
final CTA, footer).

## Stack

- **SvelteKit 2** + **Svelte 5** (runes-style state)
- **`@sveltejs/adapter-cloudflare`** — single deploy target: Cloudflare Pages
- **No CSS framework.** Hand-written CSS using the design's token system
  (paper / snow / ink themes, density modifiers, oklch accent).
- **Tweaks panel** ported from React → Svelte. Persisted to localStorage.

## Local dev

```bash
cd web
pnpm install   # or npm install
pnpm dev
```

Open <http://localhost:5173>. The live demo runs entirely in-browser using
FNV-1a — no backend required.

## Wire the real fingerprint API

The `/api/fingerprint` route proxies to your running `ucfp` HTTP server
(`POST /v1/ingest/text`). It uses the bearer-token auth contract from
`bin/ucfp.rs`.

Set in `.dev.vars` (local) or in the Cloudflare Pages dashboard (prod):

```
UCFP_API_URL=https://api.your-ucfp.dev
UCFP_API_TOKEN=<bearer matching UCFP_TOKEN on the backend>
```

When unset, the client falls back to the in-browser demo so the page stays
deployable as pure-static for previews.

## Deploy to Cloudflare Pages

```bash
pnpm build
pnpm deploy
```

`adapter-cloudflare` emits a `_worker.js` so SvelteKit endpoints (the
`/api/*` routes) run as Pages Functions. Static prerendered HTML is served
from the edge cache.

## Layout

```
src/
├── app.html             # shell document
├── app.css              # full design tokens + components + themes
├── lib/
│   ├── components/      # Nav, Hero, Demo, Usage, ... Tweaks
│   ├── stores/          # tweaks state (Svelte 5 runes)
│   └── utils/           # in-browser FNV-1a fingerprint fallback
└── routes/
    ├── +layout.svelte   # mounts Tweaks panel + global CSS
    ├── +page.svelte     # composes all landing-page sections
    └── api/fingerprint/ # SvelteKit endpoint → ucfp Rust backend
```
