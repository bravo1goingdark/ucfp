// API key generation, listing, and revocation.
//
// Storage contract: D1 keeps `prefix` (display-only) + `key_hash` (sha256 of
// the plaintext token). The plaintext is returned to the caller EXACTLY ONCE
// in the create response — we cannot recover it after that.
//
// Token shape: `ucfp_` + 43 chars of base64url(random32). Prefix shape:
// `ucfp_` + first 8 chars after the literal `ucfp_`, e.g. `ucfp_3f9a1b2c`.
// Both align with the migration comment + dashboard rendering.

import type { D1Database } from '@cloudflare/workers-types';
import { randomBytes, sha256Hex } from './auth';
import {
  insertApiKey,
  listApiKeysByUser,
  revokeApiKey,
  countActiveApiKeys,
  type ApiKeyRow
} from './db';
import type { CreatedKey, KeyRow } from '$lib/types/api';

const TOKEN_PREFIX = 'ucfp_';
const PREFIX_RANDOM_LEN = 8;

function bytesToB64Url(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export interface GeneratedApiKey {
  /** Plaintext token to surface to the user once. */
  token: string;
  /** Display prefix (`ucfp_<8>`) — safe to store and show in lists. */
  prefix: string;
  /** sha256 hex of `token`. Stored in `api_keys.key_hash`. */
  hash: string;
}

export async function generateApiKey(): Promise<GeneratedApiKey> {
  const random = bytesToB64Url(randomBytes(32));
  const token = TOKEN_PREFIX + random;
  const prefix = TOKEN_PREFIX + random.slice(0, PREFIX_RANDOM_LEN);
  const hash = await sha256Hex(token);
  return { token, prefix, hash };
}

/** Map a D1 row to the public TS shape (snake → camel; drop `key_hash`). */
export function toKeyRow(row: ApiKeyRow): KeyRow {
  return {
    id: row.id,
    name: row.name,
    prefix: row.prefix,
    createdAt: row.created_at,
    lastUsedAt: row.last_used_at,
    revokedAt: row.revoked_at,
    rateLimitPerMin: row.rate_limit_per_min,
    dailyQuota: row.daily_quota
  };
}

/** Cheap UUID source that doesn't need the Web Crypto subtle path. */
function newKeyId(): string {
  // Workers expose `crypto.randomUUID()` per Web Crypto.
  return crypto.randomUUID();
}

export interface CreateApiKeyArgs {
  db: D1Database;
  userId: string;
  name: string;
  rateLimitPerMin?: number;
  dailyQuota?: number;
}

export async function createApiKey(
  args: CreateApiKeyArgs
): Promise<CreatedKey> {
  const generated = await generateApiKey();
  const id = newKeyId();
  const createdAt = Math.floor(Date.now() / 1000);
  const row = await insertApiKey(args.db, {
    id,
    userId: args.userId,
    name: args.name,
    prefix: generated.prefix,
    keyHash: generated.hash,
    rateLimitPerMin: args.rateLimitPerMin,
    dailyQuota: args.dailyQuota,
    createdAt
  });
  return { ...toKeyRow(row), token: generated.token };
}

export async function listKeys(
  db: D1Database,
  userId: string
): Promise<KeyRow[]> {
  const rows = await listApiKeysByUser(db, userId);
  return rows.map(toKeyRow);
}

/** Returns true when the row was revoked just now, false on miss / already-revoked. */
export async function revokeKey(
  db: D1Database,
  args: { id: string; userId: string }
): Promise<boolean> {
  return revokeApiKey(db, {
    id: args.id,
    userId: args.userId,
    revokedAt: Math.floor(Date.now() / 1000)
  });
}

export async function countActiveKeys(
  db: D1Database,
  userId: string
): Promise<number> {
  return countActiveApiKeys(db, userId);
}
