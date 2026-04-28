# UCFP — Deployment Runbook

End-to-end steps to go from a fresh Cloudflare account to a fully running UCFP
production stack (Rust API on CF Containers + SvelteKit on CF Pages).

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| `wrangler` | ≥ 3.x | `npm install -g wrangler@latest` |
| `docker` + `buildx` | any | Docker Desktop or `apt install docker.io` |
| `gh` (GitHub CLI) | ≥ 2.x | https://cli.github.com |
| Cloudflare account | — | https://dash.cloudflare.com |
| GitHub repo | — | fork / clone this repo |

```bash
wrangler login          # browser OAuth — authorize CF account
gh auth login           # browser OAuth — authorize GitHub account
```

---

## 1 — Cloudflare resources

```bash
# D1 database (SvelteKit auth + usage)
wrangler d1 create ucfp_web
# ⇒ note the database_id printed; paste it into web/wrangler.toml → [[d1_databases]].database_id

# KV namespace (session cache + rate-limit counters)
wrangler kv:namespace create RATE_LIMIT
# ⇒ note the id; paste into web/wrangler.toml → [[kv_namespaces]].id

# R2 bucket (file uploads from the demo)
wrangler r2 bucket create ucfp-uploads

# Analytics Engine (usage telemetry — no provisioning step needed;
# the binding in wrangler.toml is enough)
```

### 1a — Patch wrangler.toml with real IDs

```bash
# web/wrangler.toml lines to update:
#   [[d1_databases]]  database_id = "<paste d1 id>"
#   [[kv_namespaces]] id          = "<paste kv id>"
```

---

## 2 — Apply D1 migrations

```bash
cd web
wrangler d1 migrations apply ucfp_web --remote
# Expected output: ✔ Migrations applied successfully
```

---

## 3 — Set secrets

```bash
# Required
wrangler secret put UCFP_API_URL        # e.g. https://ucfp-api.<your-cf-subdomain>.workers.dev
wrangler secret put UCFP_API_TOKEN      # shared service bearer between Pages ↔ Container
wrangler secret put SESSION_SECRET      # 32+ random bytes, e.g. `openssl rand -hex 32`

# Optional (Turnstile — enables bot protection on demo + signup)
wrangler secret put TURNSTILE_SECRET
# PUBLIC_TURNSTILE_SITE_KEY goes into wrangler.toml [vars], not secrets

# Optional (usage/rate-limit webhooks)
# wrangler secret put UCFP_RATELIMIT_URL
# wrangler secret put UCFP_USAGE_WEBHOOK_URL
```

---

## 4 — Deploy SvelteKit app to Cloudflare Pages

```bash
cd web
pnpm install --frozen-lockfile
pnpm build

wrangler pages deploy .svelte-kit/cloudflare \
  --project-name ucfp-web \
  --branch main
# First run: wrangler creates the Pages project automatically.
# Subsequent runs: CI does this via .github/workflows/web.yml
```

After deploy, set a custom domain in the CF Pages dashboard if desired (Settings → Custom domains).

---

## 5 — Build and push the Rust container image

The `container.yml` workflow does this automatically whenever the `rust` workflow
passes on `main`. To do it manually:

```bash
# Log in to GHCR
echo $GITHUB_TOKEN | docker login ghcr.io -u <github-username> --password-stdin

# Build multi-arch image (amd64 for CF, arm64 for local dev on Apple silicon)
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag ghcr.io/<github-username>/ucfp:latest \
  --push \
  .
```

---

## 6 — Deploy the Rust Container

```bash
# Update container.toml with your account ID and the correct image reference
# (the CI workflow does this automatically via sed)
sed -i "s|account_id = .*|account_id = \"<your-cf-account-id>\"|" container.toml
sed -i "s|image = .*|image = \"ghcr.io/<github-username>/ucfp:latest\"|" container.toml

wrangler container deploy --config container.toml

# Set runtime secrets inside the container
wrangler secret put UCFP_TOKEN --config container.toml   # same value as UCFP_API_TOKEN above
```

### 6a — Persistent storage (redb snapshots to R2)

The container writes its DB to `/data/ucfp.redb`. To survive restarts, set up
the R2 snapshot loop:

```bash
# Create an R2 bucket for snapshots
wrangler r2 bucket create ucfp-redb-snapshots

# Generate an R2 API token (CF dashboard → R2 → Manage R2 API Tokens)
# Then set these on the container:
wrangler secret put AWS_ACCESS_KEY_ID     --config container.toml
wrangler secret put AWS_SECRET_ACCESS_KEY --config container.toml
# Also set in container.toml [vars]:
#   R2_ENDPOINT = "https://<accountid>.r2.cloudflarestorage.com"
#   R2_BUCKET   = "ucfp-redb-snapshots"
```

---

## 7 — Smoke test production

```bash
DOMAIN=https://ucfp-web.pages.dev   # or your custom domain

# API server health
curl "$DOMAIN/status"               # SvelteKit status page (shows green pill + latency)
curl https://ucfp-api.<subdomain>.workers.dev/healthz   # direct container health

# Signup → get a session
curl -X POST "$DOMAIN/api/auth/signup" \
  -H "Content-Type: application/json" \
  -d '{"email":"you@example.com","password":"hunter2hunter2"}'

# Create an API key (copy the token from the response — shown only once)
curl -X POST "$DOMAIN/api/keys" \
  -H "Cookie: <session-cookie>" \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'

# Use the key
curl -X POST "$DOMAIN/api/fingerprint" \
  -H "X-Api-Key: ucfp_<token>" \
  -H "Content-Type: text/plain" \
  -d "Hello, production!"

# Usage should appear within ~30s
curl "$DOMAIN/api/usage?days=1" -H "Cookie: <session-cookie>"
```

---

## 8 — GitHub Actions secrets (one-time setup)

Navigate to your repo → Settings → Secrets and variables → Actions:

| Secret | Value |
|---|---|
| `CF_API_TOKEN` | Cloudflare API token with *Containers:Edit* + *Pages:Edit* |
| `CF_ACCOUNT_ID` | Your Cloudflare account ID |
| `CLOUDFLARE_API_TOKEN` | Same token (used by wrangler in web.yml) |

The `container.yml` workflow also uses the built-in `GITHUB_TOKEN` for GHCR push
(no extra secret needed for repos owned by the same account).

---

## Rollback

**Web (Pages):** Go to CF Pages dashboard → Deployments → pick an older deployment → Rollback.

**Container:** Re-run `wrangler container deploy` with a previous image digest:
```bash
wrangler container deploy --config container.toml \
  # edit container.toml image = "ghcr.io/.../ucfp@sha256:<previous-digest>"
```

---

## Environment variables reference

### Rust container (`container.toml [vars]` / secrets)

| Var | Required | Default | Effect |
|---|---|---|---|
| `UCFP_TOKEN` | one of three | — | Single service bearer (use this for Pages→Container) |
| `UCFP_KEYS_FILE` | one of three | — | TOML multi-tenant key map |
| `UCFP_KEY_LOOKUP_URL` | one of three | — | Webhook key lookup (multi-tenant feature) |
| `UCFP_BIND` | no | `0.0.0.0:8080` | Listen address |
| `UCFP_DATA_DIR` | no | `./data` | Directory for `ucfp.redb` |
| `UCFP_BODY_LIMIT_MB` | no | `16` | Max request body size |
| `UCFP_RATELIMIT_URL` | no | in-memory | Webhook rate limiter |
| `UCFP_USAGE_LOG_PATH` | no | noop | NDJSON usage log path |
| `UCFP_USAGE_WEBHOOK_URL` | no | noop | Webhook usage sink |
| `R2_ENDPOINT` | if snapshots | — | `https://<id>.r2.cloudflarestorage.com` |
| `R2_BUCKET` | if snapshots | — | Bucket name for redb snapshots |
| `AWS_ACCESS_KEY_ID` | if snapshots | — | R2 API token key |
| `AWS_SECRET_ACCESS_KEY` | if snapshots | — | R2 API token secret |

### SvelteKit Pages (`web/wrangler.toml [vars]` / secrets)

| Var | Required | Effect |
|---|---|---|
| `UCFP_API_URL` | yes | Base URL of the Rust container |
| `UCFP_API_TOKEN` | yes | Service bearer for Pages→Container calls |
| `SESSION_SECRET` | yes | HMAC key for session cookies |
| `TURNSTILE_SECRET` | no | Enables Turnstile bot protection |
| `PUBLIC_TURNSTILE_SITE_KEY` | no | Client-side Turnstile site key |
