import { useCallback, useMemo } from 'react'
import { useConfig } from '../context/ConfigContext'
import {
  batchProcess,
  compareDocuments,
  ctxFrom,
  deleteDocument,
  getDocument,
  healthCheck,
  indexStats,
  insertToIndex,
  listDocuments,
  matchDocuments,
  metrics,
  pipelineStatus,
  processDocument,
  readinessCheck,
  searchIndex,
  serverMetadata,
  testConnection,
  type BatchProcessRequest,
  type IndexInsertRequest,
  type MatchRequestBody,
  type ProcessRequest,
} from './client'

/**
 * Bound API client. Each method reads the current server URL / API key from
 * ConfigContext at call-time, so updating config in the UI instantly reconfigures
 * every subsequent request.
 */
export function useClient() {
  const { config } = useConfig()
  const ctx = useMemo(() => ctxFrom(config), [config])

  return useMemo(
    () => ({
      ctx,
      health: () => healthCheck(ctx),
      ready: () => readinessCheck(ctx),
      metrics: () => metrics(ctx),
      pipelineStatus: () => pipelineStatus(ctx),
      serverMetadata: () => serverMetadata(ctx),
      indexStats: () => indexStats(ctx),
      testConnection: () => testConnection(ctx),
      process: (body: ProcessRequest) => processDocument(ctx, body),
      batch: (body: BatchProcessRequest) => batchProcess(ctx, body),
      insert: (body: IndexInsertRequest) => insertToIndex(ctx, body),
      search: (query: string, strategy = 'perceptual', topK = 10, tenantId?: string) =>
        searchIndex(ctx, query, strategy, topK, tenantId),
      listDocuments: () => listDocuments(ctx),
      getDocument: (docId: string) => getDocument(ctx, docId),
      deleteDocument: (docId: string) => deleteDocument(ctx, docId),
      match: (body: MatchRequestBody) => matchDocuments(ctx, body),
      compare: (
        doc1: { text: string; doc_id?: string },
        doc2: { text: string; doc_id?: string },
      ) => compareDocuments(ctx, doc1, doc2),
    }),
    [ctx],
  )
}

/**
 * Hook that builds a `ProcessRequest` pre-populated with the current config's
 * enable flags and pipeline tuning parameters. The caller provides just the text.
 */
export function useProcessRequestBuilder() {
  const { config } = useConfig()
  return useCallback(
    (text: string, overrides?: Partial<ProcessRequest>): ProcessRequest => ({
      text,
      tenant_id: config.defaultTenant || undefined,
      enable_perceptual: config.enablePerceptual,
      enable_semantic: config.enableSemantic,
      perceptual_config: config.enablePerceptual ? config.perceptual : undefined,
      semantic_config: config.enableSemantic ? sanitizeSemantic(config.semantic) : undefined,
      ...overrides,
    }),
    [config],
  )
}

function sanitizeSemantic(s: import('../context/ConfigContext').SemanticConfig) {
  // Strip empty strings that backend interprets as missing optional fields.
  const out = { ...s }
  if (!out.api_url) delete out.api_url
  if (!out.api_auth_header) delete out.api_auth_header
  if (!out.api_provider) delete out.api_provider
  return out
}
