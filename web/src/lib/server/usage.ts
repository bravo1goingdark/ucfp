// Usage event recording — D1 row + Workers Analytics Engine event.
//
// Both writes are designed to be invoked through `event.platform.context.waitUntil(...)`
// so the response isn't blocked. Each write nil-checks its binding so missing
// configuration (e.g. local dev without ANALYTICS) degrades to a no-op
// instead of throwing.

import type {
  AnalyticsEngineDataset,
  D1Database
} from '@cloudflare/workers-types';
import { insertUsageEvent } from './db';

export type Modality = 'text' | 'image' | 'audio';

export interface UsageRecord {
  userId: string;
  apiKeyId: string | null;
  modality: Modality;
  algorithm: string | null;
  status: number;
  latencyMs: number;
  bytesIn: number;
}

/** Insert into D1 `usage_events`. Swallows errors — fire-and-forget. */
export async function recordUsageInD1(
  db: D1Database,
  rec: UsageRecord
): Promise<void> {
  try {
    await insertUsageEvent(db, {
      userId: rec.userId,
      apiKeyId: rec.apiKeyId,
      modality: rec.modality,
      algorithm: rec.algorithm,
      bytesIn: rec.bytesIn,
      status: rec.status,
      latencyMs: rec.latencyMs,
      createdAt: Math.floor(Date.now() / 1000)
    });
  } catch (err) {
    console.error('usage: D1 insert failed', err);
  }
}

/**
 * Push to Workers Analytics Engine for high-cardinality dashboards.
 * `blobs[0]` is the index — keep modality there for cheap modality breakdowns.
 */
export function recordUsageInAnalytics(
  analytics: AnalyticsEngineDataset | undefined,
  rec: UsageRecord
): void {
  if (!analytics) return;
  try {
    analytics.writeDataPoint({
      blobs: [rec.modality, String(rec.status), rec.algorithm ?? ''],
      doubles: [rec.latencyMs, rec.bytesIn],
      indexes: [rec.apiKeyId ?? rec.userId]
    });
  } catch (err) {
    console.error('usage: analytics write failed', err);
  }
}

export interface RecordUsageDeps {
  db?: D1Database;
  analytics?: AnalyticsEngineDataset;
}

/**
 * Convenience: do both writes. Caller should wrap in `waitUntil`.
 *
 *   ctx.waitUntil(recordUsage({ db, analytics }, rec));
 */
export async function recordUsage(
  deps: RecordUsageDeps,
  rec: UsageRecord
): Promise<void> {
  recordUsageInAnalytics(deps.analytics, rec);
  if (deps.db) await recordUsageInD1(deps.db, rec);
}
