// GET /api/usage?days=N   → { points: UsagePoint[], summary: UsageSummary }
//
// Aggregates D1 `usage_events` over the last `days` (default 30, capped at
// 365). Both the per-day breakdown and the rollup summary are computed in
// SQL — no in-memory bucketing here.

import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import {
  aggregateUsageByDay,
  listRecentUsageEvents,
  summarizeUsage,
  type UsageEventRow
} from '$lib/server/db';
import type {
  Modality,
  UsageEvent,
  UsagePoint,
  UsageResponse,
  UsageSummary
} from '$lib/types/api';

const DEFAULT_DAYS = 30;
const MAX_DAYS = 365;
const RECENT_EVENT_LIMIT = 25;

function parseDays(raw: string | null): number {
  if (!raw) return DEFAULT_DAYS;
  const n = parseInt(raw, 10);
  if (!Number.isFinite(n) || n <= 0) return DEFAULT_DAYS;
  return Math.min(n, MAX_DAYS);
}

function toUsageEvent(row: UsageEventRow): UsageEvent {
  return {
    id: row.id,
    modality: row.modality,
    algorithm: row.algorithm,
    status: row.status,
    latencyMs: row.latency_ms,
    bytesIn: row.bytes_in,
    createdAt: row.created_at
  };
}

export const GET: RequestHandler = async ({ locals, platform, url }) => {
  if (!locals.user) error(401, 'unauthenticated');
  const env = platform?.env;
  if (!env?.DB) error(503, 'usage backend not configured');

  const days = parseDays(url.searchParams.get('days'));
  const sinceUnix = Math.floor(Date.now() / 1000) - days * 86_400;

  const [pointRows, summaryRow, recentRows] = await Promise.all([
    aggregateUsageByDay(env.DB, locals.user.id, sinceUnix),
    summarizeUsage(env.DB, locals.user.id, sinceUnix),
    listRecentUsageEvents(env.DB, locals.user.id, RECENT_EVENT_LIMIT)
  ]);

  const points: UsagePoint[] = pointRows.map((p) => ({
    day: p.day,
    modality: p.modality as Modality,
    count: Number(p.count) || 0
  }));

  const summary: UsageSummary = {
    totalRequests: Number(summaryRow.total) || 0,
    modalityBreakdown: {
      text: Number(summaryRow.text_count) || 0,
      image: Number(summaryRow.image_count) || 0,
      audio: Number(summaryRow.audio_count) || 0
    },
    errorCount: Number(summaryRow.error_count) || 0,
    recentEvents: recentRows.map(toUsageEvent)
  };

  const body: UsageResponse = { points, summary };
  return json(body);
};
