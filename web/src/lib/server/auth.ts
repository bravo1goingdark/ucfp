// Lucia-pattern auth (no library): PBKDF2-SHA256 password hash via Web Crypto
// (native in CF Workers, no WASM, ~2ms CPU), 30-day rolling sessions stored
// as sha256(token) in D1 + cached in KV.

import type { D1Database, KVNamespace } from '@cloudflare/workers-types';
import {
  findSession,
  findUserById,
  insertSession,
  deleteSession,
  refreshSessionExpiry,
  type UserRow
} from './db';

const SESSION_TTL_SECONDS = 60 * 60 * 24 * 30; // 30 days
const SESSION_REFRESH_AFTER_SECONDS = 60 * 60 * 24 * 15; // 15 days
const SESSION_KV_TTL = 60 * 5; // 5 min cache, D1 is source of truth

// ── primitives ──────────────────────────────────────────────────────────
function bytesToHex(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
  return s;
}

function bytesToB64Url(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function randomBytes(n: number): Uint8Array {
  const out = new Uint8Array(n);
  crypto.getRandomValues(out);
  return out;
}

export async function sha256Hex(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest('SHA-256', data);
  return bytesToHex(new Uint8Array(digest));
}

// ── password hashing — PBKDF2-SHA256 via Web Crypto ─────────────────────
// Format: "pbkdf2:sha256:<iterations>:<b64(salt)>:<b64(hash)>"
// 100 000 iterations ≈ 2 ms on CF Workers (native, no WASM budget).
const PBKDF2_ITERATIONS = 100_000;

function b64Encode(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function b64Decode(s: string): Uint8Array {
  return Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
}

async function pbkdf2Derive(plain: string, salt: Uint8Array, iterations: number): Promise<Uint8Array> {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey('raw', enc.encode(plain), 'PBKDF2', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    { name: 'PBKDF2', hash: 'SHA-256', salt: salt as any, iterations },
    key,
    256
  );
  return new Uint8Array(bits);
}

export async function hashPassword(plain: string): Promise<string> {
  const salt = randomBytes(16);
  const hash = await pbkdf2Derive(plain, salt, PBKDF2_ITERATIONS);
  return `pbkdf2:sha256:${PBKDF2_ITERATIONS}:${b64Encode(salt)}:${b64Encode(hash)}`;
}

export async function verifyPassword(stored: string, plain: string): Promise<boolean> {
  try {
    const parts = stored.split(':');
    if (parts.length !== 5 || parts[0] !== 'pbkdf2') return false;
    const iterations = Number(parts[2]);
    const salt = b64Decode(parts[3]);
    const expected = b64Decode(parts[4]);
    const actual = await pbkdf2Derive(plain, salt, iterations);
    if (actual.length !== expected.length) return false;
    let diff = 0;
    for (let i = 0; i < actual.length; i++) diff |= actual[i] ^ expected[i];
    return diff === 0;
  } catch {
    return false;
  }
}

// ── session token ↔ session id ──────────────────────────────────────────
// Token = 32 random bytes b64url, sent in cookie.
// Session id = sha256(token) hex, the value stored in D1 / KV.
export function generateSessionToken(): string {
  return bytesToB64Url(randomBytes(32));
}

export async function tokenToSessionId(token: string): Promise<string> {
  return sha256Hex(token);
}

// ── lifecycle ───────────────────────────────────────────────────────────
export interface CreateSessionArgs {
  db: D1Database;
  kv: KVNamespace;
  userId: string;
  ip?: string;
  userAgent?: string;
}

export interface CreatedSession {
  token: string;
  sessionId: string;
  expiresAt: number; // unix seconds
}

export async function createSession(args: CreateSessionArgs): Promise<CreatedSession> {
  const token = generateSessionToken();
  const sessionId = await tokenToSessionId(token);
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + SESSION_TTL_SECONDS;

  await insertSession(args.db, {
    id: sessionId,
    user_id: args.userId,
    expires_at: expiresAt,
    created_at: now,
    user_agent: args.userAgent ?? null,
    ip: args.ip ?? null
  });

  // Pre-warm the KV cache so the next request hits the cache.
  await args.kv.put(
    `session:${sessionId}`,
    JSON.stringify({ user_id: args.userId, expires_at: expiresAt }),
    { expirationTtl: SESSION_KV_TTL }
  );

  return { token, sessionId, expiresAt };
}

export interface ValidatedSession {
  user: UserRow;
  session: { id: string; expiresAt: number };
}

export async function validateSession(
  db: D1Database,
  kv: KVNamespace,
  token: string
): Promise<ValidatedSession | null> {
  const sessionId = await tokenToSessionId(token);
  const now = Math.floor(Date.now() / 1000);

  // Try KV first.
  let userId: string | null = null;
  let expiresAt: number | null = null;

  const cached = await kv.get(`session:${sessionId}`);
  if (cached) {
    try {
      const parsed = JSON.parse(cached) as { user_id: string; expires_at: number };
      if (parsed.expires_at > now) {
        userId = parsed.user_id;
        expiresAt = parsed.expires_at;
      }
    } catch {
      // fall through to D1
    }
  }

  if (!userId) {
    const row = await findSession(db, sessionId);
    if (!row || row.expires_at <= now) return null;
    userId = row.user_id;
    expiresAt = row.expires_at;
    // Backfill cache.
    await kv.put(
      `session:${sessionId}`,
      JSON.stringify({ user_id: row.user_id, expires_at: row.expires_at }),
      { expirationTtl: SESSION_KV_TTL }
    );
  }

  // Rolling expiry: if the session is past its halfway point, extend it.
  if (expiresAt! - now < SESSION_REFRESH_AFTER_SECONDS) {
    const newExpiresAt = now + SESSION_TTL_SECONDS;
    await refreshSessionExpiry(db, sessionId, newExpiresAt);
    await kv.put(
      `session:${sessionId}`,
      JSON.stringify({ user_id: userId, expires_at: newExpiresAt }),
      { expirationTtl: SESSION_KV_TTL }
    );
    expiresAt = newExpiresAt;
  }

  const user = await findUserById(db, userId);
  if (!user) return null;
  return { user, session: { id: sessionId, expiresAt: expiresAt! } };
}

export async function invalidateSession(
  db: D1Database,
  kv: KVNamespace,
  token: string
): Promise<void> {
  const sessionId = await tokenToSessionId(token);
  await deleteSession(db, sessionId);
  await kv.delete(`session:${sessionId}`);
}
