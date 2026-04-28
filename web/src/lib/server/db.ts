// Thin D1 query helpers. All callers receive `D1Database` from
// `event.platform.env.DB` and pass it through; we keep no module-level state
// so multiple isolates / preview envs each get their own binding cleanly.

import type { D1Database } from '@cloudflare/workers-types';

export interface UserRow {
  id: string;
  email: string;
  password_hash: string;
  tenant_id: number;
  created_at: number;
  email_verified_at: number | null;
}

export interface SessionRow {
  id: string;
  user_id: string;
  expires_at: number;
  created_at: number;
  user_agent: string | null;
  ip: string | null;
}

export interface ApiKeyRow {
  id: string;
  user_id: string;
  name: string;
  prefix: string;
  key_hash: string;
  rate_limit_per_min: number;
  daily_quota: number;
  created_at: number;
  last_used_at: number | null;
  revoked_at: number | null;
}

export async function findUserByEmail(db: D1Database, email: string): Promise<UserRow | null> {
  const row = await db
    .prepare('SELECT * FROM users WHERE email = ?1 LIMIT 1')
    .bind(email)
    .first<UserRow>();
  return row ?? null;
}

export async function findUserById(db: D1Database, id: string): Promise<UserRow | null> {
  const row = await db
    .prepare('SELECT * FROM users WHERE id = ?1 LIMIT 1')
    .bind(id)
    .first<UserRow>();
  return row ?? null;
}

export async function insertUser(
  db: D1Database,
  args: { id: string; email: string; passwordHash: string; createdAt: number }
): Promise<UserRow> {
  await db
    .prepare(
      `INSERT INTO users (id, email, password_hash, tenant_id, created_at)
       VALUES (?1, ?2, ?3,
               COALESCE((SELECT MAX(tenant_id) FROM users WHERE tenant_id > 0), 0) + 1,
               ?4)`
    )
    .bind(args.id, args.email, args.passwordHash, args.createdAt)
    .run();
  const row = await findUserById(db, args.id);
  if (!row) throw new Error('user insert failed');
  return row;
}

export async function findSession(db: D1Database, id: string): Promise<SessionRow | null> {
  const row = await db
    .prepare('SELECT * FROM sessions WHERE id = ?1 LIMIT 1')
    .bind(id)
    .first<SessionRow>();
  return row ?? null;
}

export async function insertSession(
  db: D1Database,
  s: SessionRow
): Promise<void> {
  await db
    .prepare(
      'INSERT INTO sessions (id, user_id, expires_at, created_at, user_agent, ip) VALUES (?1, ?2, ?3, ?4, ?5, ?6)'
    )
    .bind(s.id, s.user_id, s.expires_at, s.created_at, s.user_agent, s.ip)
    .run();
}

export async function deleteSession(db: D1Database, id: string): Promise<void> {
  await db.prepare('DELETE FROM sessions WHERE id = ?1').bind(id).run();
}

export async function refreshSessionExpiry(
  db: D1Database,
  id: string,
  expiresAt: number
): Promise<void> {
  await db
    .prepare('UPDATE sessions SET expires_at = ?1 WHERE id = ?2')
    .bind(expiresAt, id)
    .run();
}

// ── api_keys helpers ────────────────────────────────────────────────────

export interface InsertApiKeyArgs {
  id: string;
  userId: string;
  name: string;
  prefix: string;
  keyHash: string;
  rateLimitPerMin?: number;
  dailyQuota?: number;
  createdAt: number;
}

export async function insertApiKey(
  db: D1Database,
  args: InsertApiKeyArgs
): Promise<ApiKeyRow> {
  const rateLimit = args.rateLimitPerMin ?? 600;
  const quota = args.dailyQuota ?? 50_000;
  await db
    .prepare(
      `INSERT INTO api_keys
         (id, user_id, name, prefix, key_hash, rate_limit_per_min, daily_quota, created_at)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)`
    )
    .bind(
      args.id,
      args.userId,
      args.name,
      args.prefix,
      args.keyHash,
      rateLimit,
      quota,
      args.createdAt
    )
    .run();
  const row = await db
    .prepare('SELECT * FROM api_keys WHERE id = ?1 LIMIT 1')
    .bind(args.id)
    .first<ApiKeyRow>();
  if (!row) throw new Error('api key insert failed');
  return row;
}

/** All keys (active + revoked) belonging to a user, newest first. */
export async function listApiKeysByUser(
  db: D1Database,
  userId: string
): Promise<ApiKeyRow[]> {
  const res = await db
    .prepare('SELECT * FROM api_keys WHERE user_id = ?1 ORDER BY created_at DESC')
    .bind(userId)
    .all<ApiKeyRow>();
  return res.results ?? [];
}

/**
 * Soft-delete: set `revoked_at` if the key belongs to `userId` and is not
 * already revoked. Returns true if a row was updated, false otherwise.
 * Hard-delete would null `usage_events.api_key_id` via FK and lose audit.
 */
export async function revokeApiKey(
  db: D1Database,
  args: { id: string; userId: string; revokedAt: number }
): Promise<boolean> {
  const res = await db
    .prepare(
      `UPDATE api_keys
          SET revoked_at = ?1
        WHERE id = ?2 AND user_id = ?3 AND revoked_at IS NULL`
    )
    .bind(args.revokedAt, args.id, args.userId)
    .run();
  // D1 surfaces `meta.changes`; treat anything > 0 as a hit.
  const changes = (res.meta as { changes?: number } | undefined)?.changes ?? 0;
  return changes > 0;
}

export interface ApiKeyWithUser {
  key: ApiKeyRow;
  user: UserRow;
}

/**
 * Lookup an active (not revoked) API key by its sha256 hash, joined to the
 * owning user so callers get `tenant_id` + `id` in one round trip.
 */
export async function findApiKeyWithUserByHash(
  db: D1Database,
  keyHash: string
): Promise<ApiKeyWithUser | null> {
  const row = await db
    .prepare(
      `SELECT
         k.id                 AS k_id,
         k.user_id            AS k_user_id,
         k.name               AS k_name,
         k.prefix             AS k_prefix,
         k.key_hash           AS k_key_hash,
         k.rate_limit_per_min AS k_rate_limit_per_min,
         k.daily_quota        AS k_daily_quota,
         k.created_at         AS k_created_at,
         k.last_used_at       AS k_last_used_at,
         k.revoked_at         AS k_revoked_at,
         u.id                 AS u_id,
         u.email              AS u_email,
         u.password_hash      AS u_password_hash,
         u.tenant_id          AS u_tenant_id,
         u.created_at         AS u_created_at,
         u.email_verified_at  AS u_email_verified_at
       FROM api_keys k
       JOIN users u ON u.id = k.user_id
       WHERE k.key_hash = ?1 AND k.revoked_at IS NULL
       LIMIT 1`
    )
    .bind(keyHash)
    .first<Record<string, unknown>>();
  if (!row) return null;
  return {
    key: {
      id: row.k_id as string,
      user_id: row.k_user_id as string,
      name: row.k_name as string,
      prefix: row.k_prefix as string,
      key_hash: row.k_key_hash as string,
      rate_limit_per_min: row.k_rate_limit_per_min as number,
      daily_quota: row.k_daily_quota as number,
      created_at: row.k_created_at as number,
      last_used_at: (row.k_last_used_at as number | null) ?? null,
      revoked_at: (row.k_revoked_at as number | null) ?? null
    },
    user: {
      id: row.u_id as string,
      email: row.u_email as string,
      password_hash: row.u_password_hash as string,
      tenant_id: row.u_tenant_id as number,
      created_at: row.u_created_at as number,
      email_verified_at: (row.u_email_verified_at as number | null) ?? null
    }
  };
}

export async function touchApiKeyLastUsed(
  db: D1Database,
  id: string,
  ts: number
): Promise<void> {
  await db
    .prepare('UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2')
    .bind(ts, id)
    .run();
}

export async function countActiveApiKeys(
  db: D1Database,
  userId: string
): Promise<number> {
  const row = await db
    .prepare(
      'SELECT COUNT(*) AS n FROM api_keys WHERE user_id = ?1 AND revoked_at IS NULL'
    )
    .bind(userId)
    .first<{ n: number }>();
  return row?.n ?? 0;
}

// ── usage_events helpers ────────────────────────────────────────────────

export interface UsageEventRow {
  id: number;
  user_id: string;
  api_key_id: string | null;
  modality: 'text' | 'image' | 'audio';
  algorithm: string | null;
  bytes_in: number;
  status: number;
  latency_ms: number;
  created_at: number;
}

export interface InsertUsageEventArgs {
  userId: string;
  apiKeyId: string | null;
  modality: 'text' | 'image' | 'audio';
  algorithm: string | null;
  bytesIn: number;
  status: number;
  latencyMs: number;
  createdAt: number;
}

export async function insertUsageEvent(
  db: D1Database,
  args: InsertUsageEventArgs
): Promise<void> {
  await db
    .prepare(
      `INSERT INTO usage_events
         (user_id, api_key_id, modality, algorithm, bytes_in, status, latency_ms, created_at)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)`
    )
    .bind(
      args.userId,
      args.apiKeyId,
      args.modality,
      args.algorithm,
      args.bytesIn,
      args.status,
      args.latencyMs,
      args.createdAt
    )
    .run();
}

export interface UsagePointRow {
  day: string;                         // 'YYYY-MM-DD' (UTC)
  modality: 'text' | 'image' | 'audio';
  count: number;
}

/** Group by (UTC day, modality) for a user since `sinceUnix` (seconds). */
export async function aggregateUsageByDay(
  db: D1Database,
  userId: string,
  sinceUnix: number
): Promise<UsagePointRow[]> {
  const res = await db
    .prepare(
      `SELECT date(created_at, 'unixepoch') AS day,
              modality                        AS modality,
              COUNT(*)                        AS count
         FROM usage_events
        WHERE user_id = ?1 AND created_at >= ?2
        GROUP BY day, modality
        ORDER BY day ASC`
    )
    .bind(userId, sinceUnix)
    .all<UsagePointRow>();
  return res.results ?? [];
}

export interface UsageSummaryRow {
  total: number;
  text_count: number;
  image_count: number;
  audio_count: number;
  error_count: number;
}

export async function summarizeUsage(
  db: D1Database,
  userId: string,
  sinceUnix: number
): Promise<UsageSummaryRow> {
  const row = await db
    .prepare(
      `SELECT
          COUNT(*)                                              AS total,
          SUM(CASE WHEN modality = 'text'  THEN 1 ELSE 0 END)   AS text_count,
          SUM(CASE WHEN modality = 'image' THEN 1 ELSE 0 END)   AS image_count,
          SUM(CASE WHEN modality = 'audio' THEN 1 ELSE 0 END)   AS audio_count,
          SUM(CASE WHEN status >= 400      THEN 1 ELSE 0 END)   AS error_count
         FROM usage_events
        WHERE user_id = ?1 AND created_at >= ?2`
    )
    .bind(userId, sinceUnix)
    .first<UsageSummaryRow>();
  return (
    row ?? {
      total: 0,
      text_count: 0,
      image_count: 0,
      audio_count: 0,
      error_count: 0
    }
  );
}

export async function listRecentUsageEvents(
  db: D1Database,
  userId: string,
  limit: number
): Promise<UsageEventRow[]> {
  const res = await db
    .prepare(
      `SELECT * FROM usage_events
        WHERE user_id = ?1
        ORDER BY created_at DESC
        LIMIT ?2`
    )
    .bind(userId, limit)
    .all<UsageEventRow>();
  return res.results ?? [];
}
