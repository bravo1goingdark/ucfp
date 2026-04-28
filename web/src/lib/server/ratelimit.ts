// KV-backed rate limit + daily quota counters.
//
// KV is *not* atomic; we follow the standard read/increment/put pattern. A
// few extra requests slipping through under contention is acceptable for a
// best-effort throttle — the source of truth is the key's quota in D1, and
// we never return false-positives (we only ever undercount).
//
// Key shapes (all binding `RATE_LIMIT`):
//   demo:<ip>:<floor(epoch_seconds/60)>      — anonymous demo (60/min)
//   key:<keyId>:<floor(epoch_seconds/60)>    — per-key minute window
//   key:<keyId>:day:<yyyymmdd>               — per-key daily quota

import type { KVNamespace } from '@cloudflare/workers-types';

export const DEMO_LIMIT_PER_MIN = 60;
const KV_MINUTE_TTL_SECONDS = 120; // covers active minute + jitter
const KV_DAILY_TTL_SECONDS = 90_000; // ~25h

export interface LimitDecision {
  ok: boolean;
  /** Tokens left in this window (0 when denied). */
  remaining: number;
  /** Seconds until the window resets — only meaningful when `ok=false`. */
  retryAfter: number;
}

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function currentMinute(): number {
  return Math.floor(nowSeconds() / 60);
}

function utcDayKey(): string {
  const d = new Date();
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, '0');
  const day = String(d.getUTCDate()).padStart(2, '0');
  return `${y}${m}${day}`;
}

async function bumpCounter(
  kv: KVNamespace,
  key: string,
  limit: number,
  ttlSeconds: number
): Promise<{ count: number; allowed: boolean }> {
  const raw = await kv.get(key);
  const current = raw ? parseInt(raw, 10) || 0 : 0;
  if (current >= limit) {
    return { count: current, allowed: false };
  }
  const next = current + 1;
  await kv.put(key, String(next), { expirationTtl: ttlSeconds });
  return { count: next, allowed: true };
}

/**
 * Per-IP demo limit for unauthenticated calls (e.g. landing-page demo).
 * Default 60 requests/minute; matches the W3 spec.
 */
export async function checkDemoLimit(
  kv: KVNamespace,
  ip: string,
  limit = DEMO_LIMIT_PER_MIN
): Promise<LimitDecision> {
  const minute = currentMinute();
  const key = `demo:${ip}:${minute}`;
  const { count, allowed } = await bumpCounter(kv, key, limit, KV_MINUTE_TTL_SECONDS);
  if (!allowed) {
    const retryAfter = (minute + 1) * 60 - nowSeconds();
    return { ok: false, remaining: 0, retryAfter: Math.max(retryAfter, 1) };
  }
  return { ok: true, remaining: Math.max(limit - count, 0), retryAfter: 0 };
}

/** Per-key minute throttle (`rate_limit_per_min` from D1). */
export async function checkKeyMinuteLimit(
  kv: KVNamespace,
  keyId: string,
  limitPerMin: number
): Promise<LimitDecision> {
  const minute = currentMinute();
  const key = `key:${keyId}:${minute}`;
  const { count, allowed } = await bumpCounter(kv, key, limitPerMin, KV_MINUTE_TTL_SECONDS);
  if (!allowed) {
    const retryAfter = (minute + 1) * 60 - nowSeconds();
    return { ok: false, remaining: 0, retryAfter: Math.max(retryAfter, 1) };
  }
  return { ok: true, remaining: Math.max(limitPerMin - count, 0), retryAfter: 0 };
}

/** Per-key daily quota (`daily_quota` from D1). UTC day buckets. */
export async function checkKeyDailyQuota(
  kv: KVNamespace,
  keyId: string,
  dailyQuota: number
): Promise<LimitDecision> {
  const day = utcDayKey();
  const key = `key:${keyId}:day:${day}`;
  const { count, allowed } = await bumpCounter(kv, key, dailyQuota, KV_DAILY_TTL_SECONDS);
  if (!allowed) {
    // Seconds until next UTC midnight.
    const now = new Date();
    const tomorrow = Date.UTC(
      now.getUTCFullYear(),
      now.getUTCMonth(),
      now.getUTCDate() + 1
    );
    const retryAfter = Math.max(Math.floor((tomorrow - now.getTime()) / 1000), 1);
    return { ok: false, remaining: 0, retryAfter };
  }
  return { ok: true, remaining: Math.max(dailyQuota - count, 0), retryAfter: 0 };
}
