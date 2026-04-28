import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { verifyPassword, createSession } from '$lib/server/auth';
import { setSessionCookie } from '$lib/server/cookies';
import { findUserByEmail } from '$lib/server/db';

// In-memory per-IP login throttle. Worker isolates aren't shared across
// regions, so this is best-effort; the real backstop is KV in $w3 but for
// login we keep it dumb to avoid an extra round-trip on the cold path.
const recent = new Map<string, { count: number; resetAt: number }>();
const WINDOW_MS = 60_000;
const MAX_PER_WINDOW = 10;

function throttle(ip: string): boolean {
  const now = Date.now();
  const cur = recent.get(ip);
  if (!cur || cur.resetAt < now) {
    recent.set(ip, { count: 1, resetAt: now + WINDOW_MS });
    return true;
  }
  if (cur.count >= MAX_PER_WINDOW) return false;
  cur.count++;
  return true;
}

export const POST: RequestHandler = async ({ request, cookies, platform, getClientAddress }) => {
  const env = platform?.env;
  if (!env?.DB || !env?.RATE_LIMIT || !env?.SESSION_SECRET) {
    error(503, 'auth backend not configured');
  }

  const ip = getClientAddress();
  if (!throttle(ip)) error(429, 'too many login attempts');

  const body = await request.json().catch(() => null);
  if (!body || typeof body !== 'object') error(400, 'invalid json body');
  const email = String((body as Record<string, unknown>).email ?? '').trim().toLowerCase();
  const password = String((body as Record<string, unknown>).password ?? '');

  if (!email || !password) error(400, 'email and password required');

  const user = await findUserByEmail(env.DB, email);
  // Constant-time-ish: always run argon2 verify even on missing user.
  const ok = user
    ? await verifyPassword(user.password_hash, password)
    : await verifyPassword(
        '$argon2id$v=19$m=65536,t=3,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
        password
      );

  if (!user || !ok) error(401, 'invalid credentials');

  const session = await createSession({
    db: env.DB,
    kv: env.RATE_LIMIT,
    userId: user.id,
    ip,
    userAgent: request.headers.get('user-agent') ?? undefined
  });
  await setSessionCookie(cookies, env.SESSION_SECRET, session.token, session.expiresAt);

  return json({ id: user.id, email: user.email, tenantId: user.tenant_id });
};
