// Bearer / X-Api-Key middleware for `/api/fingerprint`.
//
// Flow:
//   1. Extract token from `Authorization: Bearer …` or `X-Api-Key: …`.
//   2. sha256 → D1 `api_keys` row joined to `users`.
//   3. Enforce per-key minute rate limit + per-key daily quota in KV.
//   4. Return resolved identity + per-key budgets.
//
// On the happy path the caller should `event.platform.context.waitUntil(...)`
// the helpers in `./usage.ts` AND a `touchApiKeyLastUsed` call so the
// response isn't blocked.
//
// W5 owns the call-site; we only export the middleware here.

import type { RequestEvent } from '@sveltejs/kit';
import { sha256Hex } from './auth';
import { findApiKeyWithUserByHash } from './db';
import {
  checkKeyDailyQuota,
  checkKeyMinuteLimit,
  type LimitDecision
} from './ratelimit';

const BEARER_PREFIX = 'bearer ';
const TOKEN_PREFIX = 'ucfp_';

export interface ApiKeyAuthSuccess {
  ok: true;
  user: SessionUser;            // resolved from joined `users` row
  keyId: string;
  rateLimitPerMin: number;
  dailyQuota: number;
  /** Remaining tokens in the current minute window (post-decrement). */
  minuteRemaining: number;
  /** Remaining tokens in today's quota window (post-decrement). */
  dailyRemaining: number;
}

export interface ApiKeyAuthFailure {
  ok: false;
  status: number;
  message: string;
  /** When the failure is rate-limit related, surface seconds until reset. */
  retryAfter?: number;
}

export type ApiKeyAuthResult = ApiKeyAuthSuccess | ApiKeyAuthFailure;

/** Extract a token from `Authorization: Bearer …` or `X-Api-Key`. */
export function extractApiKey(headers: Headers): string | null {
  const auth = headers.get('authorization');
  if (auth) {
    const lower = auth.toLowerCase();
    if (lower.startsWith(BEARER_PREFIX)) {
      const tok = auth.slice(BEARER_PREFIX.length).trim();
      if (tok) return tok;
    }
  }
  const xKey = headers.get('x-api-key');
  if (xKey) {
    const tok = xKey.trim();
    if (tok) return tok;
  }
  return null;
}

function denyRateLimit(reason: string, decision: LimitDecision): ApiKeyAuthFailure {
  return {
    ok: false,
    status: 429,
    message: reason,
    retryAfter: decision.retryAfter
  };
}

/**
 * Authenticate + budget-check an API request. Returns a discriminated union
 * so callers can branch on `.ok` and forward the failure verbatim.
 *
 * Pure function relative to the response — does NOT write usage events or
 * `last_used_at`; that's the caller's job (so it can include the upstream
 * status code in the usage record).
 */
export async function authenticateApiKey(
  event: RequestEvent
): Promise<ApiKeyAuthResult> {
  const env = event.platform?.env;
  if (!env?.DB || !env?.RATE_LIMIT) {
    return { ok: false, status: 503, message: 'auth backend not configured' };
  }

  const presented = extractApiKey(event.request.headers);
  if (!presented) {
    return { ok: false, status: 401, message: 'missing api key' };
  }
  if (!presented.startsWith(TOKEN_PREFIX)) {
    return { ok: false, status: 401, message: 'invalid api key format' };
  }

  const hash = await sha256Hex(presented);
  const row = await findApiKeyWithUserByHash(env.DB, hash);
  if (!row) {
    return { ok: false, status: 401, message: 'invalid or revoked api key' };
  }

  const minute = await checkKeyMinuteLimit(env.RATE_LIMIT, row.key.id, row.key.rate_limit_per_min);
  if (!minute.ok) return denyRateLimit('rate limit exceeded', minute);

  const daily = await checkKeyDailyQuota(env.RATE_LIMIT, row.key.id, row.key.daily_quota);
  if (!daily.ok) return denyRateLimit('daily quota exceeded', daily);

  return {
    ok: true,
    user: {
      id: row.user.id,
      email: row.user.email,
      tenantId: row.user.tenant_id
    },
    keyId: row.key.id,
    rateLimitPerMin: row.key.rate_limit_per_min,
    dailyQuota: row.key.daily_quota,
    minuteRemaining: minute.remaining,
    dailyRemaining: daily.remaining
  };
}
