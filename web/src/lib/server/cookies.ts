// HMAC-signed cookie helpers. The session token itself doesn't need
// signing (validateSession already checks D1), but signing lets us reject
// tampered cookies before any DB hit.

import type { Cookies } from '@sveltejs/kit';

export const SESSION_COOKIE = 'ucfp_session';

const COOKIE_OPTS = {
  path: '/',
  httpOnly: true,
  secure: true,
  sameSite: 'lax' as const
};

async function hmacSign(secret: string, message: string): Promise<string> {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    'raw',
    enc.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );
  const sig = await crypto.subtle.sign('HMAC', key, enc.encode(message));
  // base64url
  let bin = '';
  const view = new Uint8Array(sig);
  for (let i = 0; i < view.length; i++) bin += String.fromCharCode(view[i]);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function constantTimeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let r = 0;
  for (let i = 0; i < a.length; i++) r |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return r === 0;
}

export async function setSessionCookie(
  cookies: Cookies,
  secret: string,
  token: string,
  expiresAtSeconds: number
): Promise<void> {
  const sig = await hmacSign(secret, token);
  cookies.set(SESSION_COOKIE, `${token}.${sig}`, {
    ...COOKIE_OPTS,
    expires: new Date(expiresAtSeconds * 1000)
  });
}

export async function readSessionCookie(
  cookies: Cookies,
  secret: string
): Promise<string | null> {
  const raw = cookies.get(SESSION_COOKIE);
  if (!raw) return null;
  const dot = raw.lastIndexOf('.');
  if (dot < 1) return null;
  const token = raw.slice(0, dot);
  const presented = raw.slice(dot + 1);
  const expected = await hmacSign(secret, token);
  if (!constantTimeEqual(presented, expected)) return null;
  return token;
}

export function clearSessionCookie(cookies: Cookies): void {
  cookies.delete(SESSION_COOKIE, { path: '/' });
}
