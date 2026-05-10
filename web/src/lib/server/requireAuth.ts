// Shared identity gate for the new dashboard API routes
// (records, search). Resolves to `{tenantId, userId, keyId}` for either
// session or API-key auth, otherwise returns a Response with 401/429/503
// the caller can return as-is.
//
// Mirrors the identity flow in `/api/fingerprint/+server.ts` but rejects
// the anonymous demo path — record management against tenant_id=0 is
// meaningless and would let any caller scribble on the demo tenant.

import { json, type RequestEvent } from '@sveltejs/kit';
import { authenticateApiKey, extractApiKey } from './apikeyAuth';
import { checkSessionMinuteLimit } from './ratelimit';

export interface AuthedIdentity {
  tenantId: number;
  userId: string;
  keyId: string | null;
}

export type RequireAuthResult =
  | { ok: true; identity: AuthedIdentity }
  | { ok: false; response: Response };

export async function requireAuth(event: RequestEvent): Promise<RequireAuthResult> {
  const presentedKey = extractApiKey(event.request.headers);
  if (presentedKey) {
    const auth = await authenticateApiKey(event);
    if (!auth.ok) {
      const headers: Record<string, string> = {};
      if (auth.retryAfter) headers['retry-after'] = String(auth.retryAfter);
      return {
        ok: false,
        response: json(
          { error: auth.status === 429 ? 'rate_limited' : 'unauthorized', message: auth.message },
          { status: auth.status, headers }
        )
      };
    }
    return { ok: true, identity: { tenantId: auth.user.tenantId, userId: auth.user.id, keyId: auth.keyId } };
  }
  if (event.locals.user) {
    const kv = event.platform?.env?.RATE_LIMIT;
    if (kv) {
      const decision = await checkSessionMinuteLimit(kv, event.locals.user.id);
      if (!decision.ok) {
        return {
          ok: false,
          response: json(
            { error: 'rate_limited', message: 'rate limit exceeded' },
            { status: 429, headers: { 'retry-after': String(decision.retryAfter) } }
          )
        };
      }
    }
    return {
      ok: true,
      identity: { tenantId: event.locals.user.tenantId, userId: event.locals.user.id, keyId: null }
    };
  }
  return {
    ok: false,
    response: json({ error: 'unauthorized', message: 'authentication required' }, { status: 401 })
  };
}
