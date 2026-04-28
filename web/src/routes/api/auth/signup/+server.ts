import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { hashPassword, createSession } from '$lib/server/auth';
import { setSessionCookie } from '$lib/server/cookies';
import { findUserByEmail, insertUser } from '$lib/server/db';
import { verifyTurnstile } from '$lib/server/turnstile';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const POST: RequestHandler = async ({ request, cookies, platform, getClientAddress }) => {
  const env = platform?.env;
  if (!env?.DB || !env?.RATE_LIMIT || !env?.SESSION_SECRET) {
    error(503, 'auth backend not configured');
  }

  const body = await request.json().catch(() => null);
  if (!body || typeof body !== 'object') error(400, 'invalid json body');

  const email = String((body as Record<string, unknown>).email ?? '').trim().toLowerCase();
  const password = String((body as Record<string, unknown>).password ?? '');
  const turnstileToken = ((body as Record<string, unknown>)['cf-turnstile-response'] ?? null) as
    | string
    | null;

  if (!EMAIL_RE.test(email)) error(400, 'invalid email');
  if (password.length < 10) error(400, 'password must be ≥10 characters');
  if (password.length > 200) error(400, 'password too long');

  const captcha = await verifyTurnstile(env.TURNSTILE_SECRET, turnstileToken, getClientAddress());
  if (!captcha.success) error(400, 'captcha failed');

  const existing = await findUserByEmail(env.DB, email);
  if (existing) error(409, 'email already registered');

  const passwordHash = await hashPassword(password);
  const userId = crypto.randomUUID();
  const now = Math.floor(Date.now() / 1000);

  const user = await insertUser(env.DB, { id: userId, email, passwordHash, createdAt: now });

  const session = await createSession({
    db: env.DB,
    kv: env.RATE_LIMIT,
    userId: user.id,
    ip: getClientAddress(),
    userAgent: request.headers.get('user-agent') ?? undefined
  });
  await setSessionCookie(cookies, env.SESSION_SECRET, session.token, session.expiresAt);

  return json({ id: user.id, email: user.email, tenantId: user.tenant_id });
};
