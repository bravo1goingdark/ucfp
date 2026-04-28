---
title: Rate limits
order: 7
description: Per-IP demo quotas, authenticated defaults, and the X-RateLimit / Retry-After headers UCFP returns.
---

# Rate limits

UCFP enforces three independent budgets: anonymous demo, authenticated per-minute, authenticated per-day. Hitting any of them returns `429 Too Many Requests` with explanatory headers.

## Anonymous demo

| Budget | Value | Scope |
| --- | --- | --- |
| Requests per minute | **60** | per IP address |
| Daily quota | none | — |
| Body size cap | 64 KiB text / 4 MiB image / 8 MiB audio | per request |

Hosted demo callers also need to clear a Cloudflare Turnstile challenge on first contact in a session. The Turnstile token is cached for 30 minutes; subsequent calls in the same session skip the challenge.

The 60 / minute counter resets each rolling minute. Reaching it returns `429` with `Retry-After: <seconds-until-next-window>`.

## Authenticated (default for new keys)

| Budget | Value | Scope |
| --- | --- | --- |
| Requests per minute | **600** | per key |
| Daily quota | **50 000** | per key |
| Body size cap | 32 MiB | per request |

Both budgets refresh independently. Hitting the per-minute budget delays you by ≤ 60 s; hitting the daily quota requires waiting until the next UTC midnight (or upgrading the key from the dashboard).

You can raise both numbers per-key in **Dashboard → Keys → Edit**. Hard upper bounds today: 6 000 / minute, 5 000 000 / day. Need more? Open an issue.

## Header semantics

Every authenticated response carries:

| Header | Meaning |
| --- | --- |
| `X-RateLimit-Limit` | The bucket size for the budget that's closest to being hit. |
| `X-RateLimit-Remaining` | Calls left in that bucket before `429`. |
| `X-RateLimit-Reset` | Unix epoch seconds when the bucket refills (per-minute) or rolls over (daily). |

When the response is itself a `429`, you also get:

| Header | Meaning |
| --- | --- |
| `Retry-After` | Wall-clock seconds until the soonest acceptable retry. Standard HTTP semantic — equivalent to RFC 9110 § 10.2.3. |

Backoff strategy: trust `Retry-After`. Do not exponentially back off on `429` — the server already knows the next available slot and tells you.

## What counts as one call

Exactly one inbound HTTP request — one POST to `/v1/ingest/…`, one GET to `/v1/records/…`, one streaming connection (for the duration the body is open). Streaming counts as **one** call regardless of how many subfingerprints the server emits.

`/api/fingerprint` (the SvelteKit proxy) counts on the SvelteKit side as one call **and** on the Rust upstream side as one. If you hit the proxy, you spend from your key budget once — the service-bearer call to the Rust upstream is not metered against you.

## Burst behaviour

The per-minute bucket is implemented as a token bucket: 600 / 60 = 10 tokens / second refill, 600 capacity. So a burst of up to 600 in the first second is allowed, then refill takes over. This matches a "smooth average of 10 / s with reasonable burst" intuition.

The daily quota is a hard counter; no burst window — once you spent 50 000, you wait for UTC midnight.

## Cost classes

In v1 every algorithm costs **1 unit**. Future versions may charge `semantic-*` more — the response will include a `units` field once that lands. Plan ahead by reading `units` if present.

## Self-hosted

The Rust binary defaults to `NoopRateLimiter` — no limits, all callers share the single `UCFP_TOKEN`. Set `UCFP_RATELIMIT_URL=…` to plug in the webhook-based limiter, or rebuild with `--features multi-tenant` and use `InMemoryTokenBucket`. See the Rust crate's `RATELIMIT.md` for the full matrix.
