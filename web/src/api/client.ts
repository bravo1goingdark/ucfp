import type { PerceptualConfig, SemanticConfig, UcfpConfig } from '../context/ConfigContext'

export interface RequestContext {
  serverUrl: string
  apiKey: string
}

export function ctxFrom(config: UcfpConfig): RequestContext {
  return { serverUrl: config.serverUrl, apiKey: config.apiKey }
}

function headers(ctx: RequestContext, extra?: Record<string, string>): Record<string, string> {
  const h: Record<string, string> = { 'Content-Type': 'application/json', ...(extra || {}) }
  if (ctx.apiKey) h['X-API-Key'] = ctx.apiKey
  return h
}

async function request<T>(ctx: RequestContext, path: string, opts?: RequestInit): Promise<T> {
  const url = `${ctx.serverUrl.replace(/\/$/, '')}${path}`
  const res = await fetch(url, {
    ...opts,
    headers: headers(ctx, opts?.headers as Record<string, string>),
  })
  if (!res.ok) {
    let message = `HTTP ${res.status}: ${res.statusText}`
    try {
      const body = await res.json()
      if (body?.error?.message) message = body.error.message
      else if (body?.message) message = body.message
    } catch {
      /* ignore */
    }
    throw new Error(message)
  }
  const ct = res.headers.get('content-type') || ''
  if (ct.includes('application/json')) return (await res.json()) as T
  return {} as T
}

// ─── Health ──────────────────────────────────────────────

export interface HealthResponse {
  status: string
  service: string
  timestamp: string
  uptime_seconds: number
}

export async function healthCheck(ctx: RequestContext): Promise<HealthResponse> {
  return request<HealthResponse>(ctx, '/health')
}

export interface ReadinessResponse {
  status: string
  service: string
  uptime_seconds: number
  components: Record<string, string>
}

export async function readinessCheck(ctx: RequestContext): Promise<ReadinessResponse> {
  return request<ReadinessResponse>(ctx, '/ready')
}

export interface MetricsResponse {
  uptime_seconds: number
  index: { total_documents: number; total_vectors: number }
  version: string
}

export async function metrics(ctx: RequestContext): Promise<MetricsResponse> {
  return request<MetricsResponse>(ctx, '/metrics')
}

export interface PipelineStatusResponse {
  status: string
  components: {
    ingest: string
    canonical: string
    perceptual: string
    semantic: string
    index: string
    matcher: string
  }
}

export async function pipelineStatus(ctx: RequestContext): Promise<PipelineStatusResponse> {
  return request<PipelineStatusResponse>(ctx, '/api/v1/pipeline/status')
}

export interface ServerMetadata {
  version: string
  uptime_seconds: number
}

export async function serverMetadata(ctx: RequestContext): Promise<ServerMetadata> {
  return request<ServerMetadata>(ctx, '/api/v1/metadata')
}

export interface IndexStatsResponse {
  total_documents: number
  with_perceptual: number
  with_semantic: number
}

export async function indexStats(ctx: RequestContext): Promise<IndexStatsResponse> {
  return request<IndexStatsResponse>(ctx, '/api/v1/index/stats')
}

// ─── Process ─────────────────────────────────────────────

export interface ProcessRequest {
  doc_id?: string
  tenant_id?: string
  text: string
  enable_perceptual?: boolean
  enable_semantic?: boolean
  perceptual_config?: PerceptualConfig
  semantic_config?: SemanticConfig
}

export interface PerceptualFingerprint {
  minhash?: number[]
  meta?: Record<string, unknown>
  [k: string]: unknown
}

export interface SemanticEmbedding {
  vector?: number[]
  doc_id?: string
  model_name?: string
  [k: string]: unknown
}

export interface ProcessResponse {
  doc_id: string
  tenant_id: string
  status: string
  canonical_hash: string | null
  perceptual_fingerprint: PerceptualFingerprint | null
  semantic_embedding: SemanticEmbedding | null
  error: string | null
}

export async function processDocument(
  ctx: RequestContext,
  body: ProcessRequest,
): Promise<ProcessResponse> {
  return request<ProcessResponse>(ctx, '/api/v1/process', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export interface BatchDocument {
  doc_id?: string
  tenant_id?: string
  text: string
}

export interface BatchProcessRequest {
  documents: BatchDocument[]
  enable_perceptual?: boolean
  enable_semantic?: boolean
}

export interface BatchProcessResponse {
  processed: number
  successful: number
  failed: number
  results: ProcessResponse[]
}

export async function batchProcess(
  ctx: RequestContext,
  body: BatchProcessRequest,
): Promise<BatchProcessResponse> {
  return request<BatchProcessResponse>(ctx, '/api/v1/batch', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

// ─── Index ───────────────────────────────────────────────

export interface IndexInsertRequest {
  doc_id: string
  tenant_id?: string
  canonical_hash: string
  perceptual_fingerprint?: number[]
  semantic_embedding?: number[]
  metadata?: Record<string, string>
}

export interface IndexInsertResponse {
  doc_id: string
  status: string
  error: string | null
}

export async function insertToIndex(
  ctx: RequestContext,
  body: IndexInsertRequest,
): Promise<IndexInsertResponse> {
  return request<IndexInsertResponse>(ctx, '/api/v1/index/insert', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export interface SearchHit {
  doc_id: string
  score: number
  tenant_id?: string
  metadata?: Record<string, unknown>
}

export interface SearchResponse {
  query: string
  strategy: string
  total_hits: number
  hits: SearchHit[]
}

export async function searchIndex(
  ctx: RequestContext,
  query: string,
  strategy = 'perceptual',
  top_k = 10,
  tenant_id?: string,
): Promise<SearchResponse> {
  const params = new URLSearchParams({ query, strategy, top_k: String(top_k) })
  if (tenant_id) params.set('tenant_id', tenant_id)
  return request<SearchResponse>(ctx, `/api/v1/index/search?${params}`)
}

export interface IndexedDoc {
  doc_id: string
  canonical_hash: string
  tenant_id?: string
  metadata: Record<string, unknown>
}

export async function listDocuments(
  ctx: RequestContext,
): Promise<{ documents: IndexedDoc[]; total: number }> {
  return request(ctx, '/api/v1/index/documents')
}

export async function getDocument(ctx: RequestContext, docId: string) {
  return request<IndexedDoc & { has_perceptual: boolean; has_semantic: boolean }>(
    ctx,
    `/api/v1/index/documents/${encodeURIComponent(docId)}`,
  )
}

export async function deleteDocument(ctx: RequestContext, docId: string) {
  return request<{ doc_id: string; status: string }>(
    ctx,
    `/api/v1/index/documents/${encodeURIComponent(docId)}`,
    { method: 'DELETE' },
  )
}

// ─── Match ───────────────────────────────────────────────

export interface MatchHit {
  doc_id: string
  score: number
  rank: number
  tenant_id?: string
  metadata?: Record<string, unknown>
}

export interface MatchRequestBody {
  query: string
  tenant_id?: string
  strategy?: 'perceptual' | 'semantic' | 'hybrid'
  max_results?: number
  oversample_factor?: number
  min_score?: number
}

export interface MatchResponse {
  query: string
  strategy: string
  total_matches: number
  matches: MatchHit[]
}

export async function matchDocuments(
  ctx: RequestContext,
  body: MatchRequestBody,
): Promise<MatchResponse> {
  return request<MatchResponse>(ctx, '/api/v1/match', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export interface CompareResponse {
  similarity_score: number
  perceptual_similarity: number | null
  semantic_similarity: number | null
}

export async function compareDocuments(
  ctx: RequestContext,
  doc1: { text: string; doc_id?: string },
  doc2: { text: string; doc_id?: string },
): Promise<CompareResponse> {
  return request<CompareResponse>(ctx, '/api/v1/compare', {
    method: 'POST',
    body: JSON.stringify({ doc1, doc2 }),
  })
}

// ─── Connection ──────────────────────────────────────────

export async function testConnection(ctx: RequestContext): Promise<boolean> {
  try {
    await healthCheck(ctx)
    return true
  } catch {
    return false
  }
}
