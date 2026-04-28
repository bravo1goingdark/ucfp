// Public API shapes for `/api/keys` and `/api/usage`.
//
// These are consumed by the dashboard pages (W2) and produced by the
// route handlers in this package (W3). Snake_case D1 rows are mapped to
// camelCase at the query-helper boundary so route + UI code stay clean.

export type Modality = 'text' | 'image' | 'audio';

/** Public projection of an `api_keys` row — never includes `key_hash`. */
export interface KeyRow {
  id: string;
  name: string;
  /** Human-recognisable prefix, e.g. `ucfp_3f9a1b2c`. Safe to display. */
  prefix: string;
  createdAt: number;        // unix seconds
  lastUsedAt: number | null;
  revokedAt: number | null;
  rateLimitPerMin: number;
  dailyQuota: number;
}

/** Returned EXACTLY ONCE from `POST /api/keys`. The `token` is plaintext. */
export interface CreatedKey extends KeyRow {
  token: string;
}

export interface UsagePoint {
  /** `yyyy-mm-dd` in UTC (the bucket boundary used by D1's `date()`). */
  day: string;
  modality: Modality;
  count: number;
}

export interface UsageEvent {
  id: number;
  modality: Modality;
  algorithm: string | null;
  status: number;
  latencyMs: number;
  bytesIn: number;
  createdAt: number;        // unix seconds
}

export interface UsageSummary {
  totalRequests: number;
  modalityBreakdown: Record<Modality, number>;
  errorCount: number;
  recentEvents: UsageEvent[];
}

export interface UsageResponse {
  points: UsagePoint[];
  summary: UsageSummary;
}

export interface DashboardSummary {
  keysActive: number;
  usage: UsageSummary;
}
