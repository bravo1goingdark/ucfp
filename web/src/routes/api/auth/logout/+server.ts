import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { invalidateSession } from '$lib/server/auth';
import { readSessionCookie, clearSessionCookie } from '$lib/server/cookies';

export const POST: RequestHandler = async ({ cookies, platform }) => {
  const env = platform?.env;
  if (env?.DB && env?.RATE_LIMIT && env?.SESSION_SECRET) {
    const token = await readSessionCookie(cookies, env.SESSION_SECRET);
    if (token) await invalidateSession(env.DB, env.RATE_LIMIT, token);
  }
  clearSessionCookie(cookies);
  return json({ ok: true });
};
