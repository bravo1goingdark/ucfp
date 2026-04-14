import { useCallback, useEffect, useMemo, useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import {
  Fingerprint, Database, Search, GitCompare, Clock, Settings,
  FileText, Hash, Layers, Trash2, Plus, Sliders,
  AlertCircle, CheckCircle, Info, ChevronDown, ChevronUp, Eye, EyeOff,
  Activity, Zap, RefreshCw,
} from 'lucide-react'
import { useClient, useProcessRequestBuilder } from '../api/useClient'
import { useConfig } from '../context/ConfigContext'
import ConfigPanel from '../components/ConfigPanel'
import type { ProcessResponse, SearchHit, MatchHit, IndexedDoc } from '../api/client'
import {
  CopyButton, ScoreGauge, StatusBadge, EmptyState, LoadingSkeleton, Toast as ToastView,
} from '../components/ui'

type Section = 'process' | 'index' | 'search' | 'compare' | 'history' | 'config'
type ToastType = 'error' | 'success' | 'info'

interface HistoryEntry {
  id: string
  type: string
  timestamp: number
  success: boolean
  summary: string
  data?: unknown
}

interface ToastItem { id: string; message: string; type: ToastType }

interface Tools {
  addToast: (m: string, t?: ToastType) => void
  addHistory: (e: Omit<HistoryEntry, 'id' | 'timestamp'>) => void
}

const inputBase =
  'w-full px-3 py-2 text-sm bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500'

const textareaBase =
  'w-full px-3 py-2 text-sm font-mono bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500 resize-y'

const btnPrimary =
  'inline-flex items-center justify-center gap-2 px-4 py-2 text-sm font-medium rounded-md bg-accent-600 hover:bg-accent-700 text-white disabled:opacity-60 disabled:cursor-not-allowed transition-colors'

const btnSecondary =
  'inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 hover:border-zinc-300 dark:hover:border-zinc-700 hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors'

export default function DashboardPage() {
  const { config } = useConfig()
  const client = useClient()
  const [section, setSection] = useState<Section>('process')
  const [connected, setConnected] = useState<boolean | null>(null)
  const [showConfig, setShowConfig] = useState(false)
  const [toasts, setToasts] = useState<ToastItem[]>([])
  const [liveStats, setLiveStats] = useState<{
    total: number; perceptual: number; semantic: number; uptime: number; version: string
  } | null>(null)
  const [history, setHistory] = useState<HistoryEntry[]>(() => {
    try { return JSON.parse(localStorage.getItem('ucfp-history') || '[]') } catch { return [] }
  })

  useEffect(() => { localStorage.setItem('ucfp-history', JSON.stringify(history)) }, [history])

  const refreshStats = useCallback(async () => {
    try {
      const [conn, stats, m] = await Promise.all([
        client.testConnection(),
        client.indexStats().catch(() => null),
        client.metrics().catch(() => null),
      ])
      setConnected(conn)
      if (stats && m) {
        setLiveStats({
          total: stats.total_documents,
          perceptual: stats.with_perceptual,
          semantic: stats.with_semantic,
          uptime: m.uptime_seconds,
          version: m.version,
        })
      } else if (stats) {
        setLiveStats(prev => ({
          total: stats.total_documents,
          perceptual: stats.with_perceptual,
          semantic: stats.with_semantic,
          uptime: prev?.uptime ?? 0,
          version: prev?.version ?? 'unknown',
        }))
      }
    } catch {
      setConnected(false)
    }
  }, [client])

  useEffect(() => {
    refreshStats()
    const interval = setInterval(refreshStats, 15000)
    return () => clearInterval(interval)
  }, [refreshStats])

  const addToast = useCallback((message: string, type: ToastType = 'error') => {
    const id = Date.now().toString() + Math.random().toString(36).slice(2, 6)
    setToasts(prev => [...prev, { id, message, type }])
    setTimeout(() => setToasts(prev => prev.filter(t => t.id !== id)), 4000)
  }, [])

  const addHistory = useCallback((entry: Omit<HistoryEntry, 'id' | 'timestamp'>) => {
    setHistory(prev => [
      { ...entry, id: Date.now().toString() + Math.random().toString(36).slice(2, 6), timestamp: Date.now() },
      ...prev,
    ].slice(0, 100))
  }, [])

  const tools: Tools = useMemo(() => ({ addToast, addHistory }), [addToast, addHistory])

  return (
    <div className="pt-16 min-h-screen">
      <AnimatePresence>
        {toasts.map(t => (
          <ToastView
            key={t.id}
            message={t.message}
            type={t.type}
            onClose={() => setToasts(prev => prev.filter(x => x.id !== t.id))}
          />
        ))}
      </AnimatePresence>

      {/* Top bar */}
      <div className="border-b border-zinc-200 dark:border-zinc-800/80 bg-white dark:bg-zinc-950">
        <div className="mx-auto max-w-7xl px-6 py-3 flex items-center justify-between flex-wrap gap-3">
          <div className="flex items-center gap-3">
            <StatusBadge connected={!!connected} />
            <code className="text-xs font-mono text-zinc-500 dark:text-zinc-500">{config.serverUrl}</code>
            {liveStats && (
              <span className="hidden sm:inline text-xs text-zinc-400 dark:text-zinc-600">
                v{liveStats.version} · up {formatUptime(liveStats.uptime)}
              </span>
            )}
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={refreshStats}
              title="Refresh"
              className="p-2 rounded-md text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-50 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
            >
              <RefreshCw size={14} />
            </button>
            <button
              onClick={() => setShowConfig(!showConfig)}
              title="Configuration"
              className={`p-2 rounded-md transition-colors ${
                showConfig
                  ? 'bg-accent-600 text-white'
                  : 'text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-50 hover:bg-zinc-100 dark:hover:bg-zinc-800'
              }`}
            >
              <Settings size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Stats strip */}
      <div className="border-b border-zinc-200 dark:border-zinc-800/80 bg-zinc-50/50 dark:bg-zinc-950/50">
        <div className="mx-auto max-w-7xl px-6 py-4 grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
          <StatCard icon={<Database size={14} />} label="Documents" value={liveStats?.total ?? '—'} />
          <StatCard icon={<Fingerprint size={14} />} label="With perceptual" value={liveStats?.perceptual ?? '—'} />
          <StatCard icon={<Layers size={14} />} label="With semantic" value={liveStats?.semantic ?? '—'} />
          <StatCard icon={<Activity size={14} />} label="Perceptual" value={config.enablePerceptual ? 'ON' : 'OFF'} active={config.enablePerceptual} />
          <StatCard icon={<Zap size={14} />} label="Semantic" value={config.enableSemantic ? 'ON' : 'OFF'} active={config.enableSemantic} />
        </div>
      </div>

      {/* Layout */}
      <div className="mx-auto max-w-7xl px-6 py-6 grid grid-cols-1 lg:grid-cols-[200px_1fr] gap-6">
        {/* Sidebar */}
        <aside className="lg:sticky lg:top-20 lg:self-start space-y-1">
          <SidebarItem icon={<FileText size={14} />} label="Process" active={section === 'process'} onClick={() => setSection('process')} />
          <SidebarItem icon={<Database size={14} />} label="Index" active={section === 'index'} onClick={() => setSection('index')} />
          <SidebarItem icon={<Search size={14} />} label="Search" active={section === 'search'} onClick={() => setSection('search')} />
          <SidebarItem icon={<GitCompare size={14} />} label="Compare" active={section === 'compare'} onClick={() => setSection('compare')} />
          <SidebarItem icon={<Clock size={14} />} label="History" active={section === 'history'} onClick={() => setSection('history')} />
          <div className="my-2 border-t border-zinc-200 dark:border-zinc-800" />
          <SidebarItem icon={<Sliders size={14} />} label="Config" active={section === 'config'} onClick={() => setSection('config')} />
        </aside>

        {/* Content */}
        <div>
          {section === 'process' && <ProcessSection tools={tools} onAfterInsert={refreshStats} />}
          {section === 'index' && <IndexSection tools={tools} />}
          {section === 'search' && <SearchSection tools={tools} />}
          {section === 'compare' && <CompareSection tools={tools} />}
          {section === 'history' && <HistorySection history={history} setHistory={setHistory} />}
          {section === 'config' && <ConfigPanel />}
        </div>
      </div>

      {/* Floating config drawer */}
      <AnimatePresence>
        {showConfig && (
          <motion.aside
            initial={{ x: 400, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 400, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 260, damping: 30 }}
            className="fixed top-16 right-0 bottom-0 w-[380px] max-w-[90vw] z-40 overflow-y-auto border-l border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 p-5"
          >
            <ConfigPanel />
          </motion.aside>
        )}
      </AnimatePresence>
    </div>
  )
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`
  return `${Math.floor(seconds / 86400)}d`
}

function StatCard({
  icon, label, value, active,
}: {
  icon: React.ReactNode
  label: string
  value: number | string
  active?: boolean
}) {
  return (
    <div className="flex items-center gap-3 px-3 py-2 rounded-lg bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800">
      <div className={`flex items-center justify-center w-7 h-7 rounded-md ${
        active === true ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
        : active === false ? 'bg-zinc-100 dark:bg-zinc-800 text-zinc-400'
        : 'bg-accent-500/10 text-accent-600 dark:text-accent-400'
      }`}>
        {icon}
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-semibold text-zinc-900 dark:text-zinc-50 truncate">{value}</div>
        <div className="text-[10px] uppercase tracking-wider text-zinc-400 dark:text-zinc-600 truncate">{label}</div>
      </div>
    </div>
  )
}

function SidebarItem({
  icon, label, active, onClick,
}: { icon: React.ReactNode; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-2 text-sm rounded-md transition-colors ${
        active
          ? 'bg-accent-600 text-white'
          : 'text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-50'
      }`}
    >
      {icon}
      <span>{label}</span>
    </button>
  )
}

function SectionWrap({ children }: { children: React.ReactNode }) {
  return <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-6">{children}</div>
}

function SectionHeader({ icon, title, desc, right }: { icon: React.ReactNode; title: string; desc?: string; right?: React.ReactNode }) {
  return (
    <div className="mb-5 flex items-start justify-between gap-3 flex-wrap">
      <div>
        <h2 className="flex items-center gap-2 text-lg font-semibold text-zinc-900 dark:text-zinc-50">
          <span className="text-accent-600 dark:text-accent-400">{icon}</span>
          {title}
        </h2>
        {desc && <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400">{desc}</p>}
      </div>
      {right}
    </div>
  )
}

// ─── Process Section ────────────────────────────────────────────────

function ProcessSection({ tools, onAfterInsert }: { tools: Tools; onAfterInsert: () => void }) {
  const client = useClient()
  const { config } = useConfig()
  const buildRequest = useProcessRequestBuilder()

  const [text, setText] = useState('')
  const [docId, setDocId] = useState('')
  const [tenantId, setTenantId] = useState('')
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<ProcessResponse | null>(null)
  const [showRaw, setShowRaw] = useState(false)
  const [autoIndex, setAutoIndex] = useState(true)

  const wordCount = text.trim() ? text.trim().split(/\s+/).length : 0
  const charCount = text.length

  const handleProcess = async () => {
    if (!text.trim()) { tools.addToast('Enter text to fingerprint', 'error'); return }
    setLoading(true)
    setResult(null)
    try {
      const req = buildRequest(text, {
        doc_id: docId || undefined,
        tenant_id: tenantId || config.defaultTenant || undefined,
      })
      const res = await client.process(req)
      setResult(res)
      tools.addToast(`Fingerprinted: ${res.doc_id}`, 'success')
      tools.addHistory({
        type: 'process', success: true,
        summary: `Processed "${text.slice(0, 50)}${text.length > 50 ? '...' : ''}"`,
        data: res,
      })

      if (autoIndex && res.canonical_hash && res.status === 'success') {
        try {
          await client.insert({
            doc_id: res.doc_id,
            tenant_id: res.tenant_id,
            canonical_hash: res.canonical_hash,
            perceptual_fingerprint: res.perceptual_fingerprint?.minhash,
            semantic_embedding: res.semantic_embedding?.vector,
          })
          tools.addToast('Indexed automatically', 'success')
          onAfterInsert()
        } catch (e) {
          tools.addToast(`Auto-index failed: ${(e as Error).message}`, 'error')
        }
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Unknown error'
      tools.addToast(msg, 'error')
      tools.addHistory({ type: 'process', success: false, summary: msg })
    } finally {
      setLoading(false)
    }
  }

  const handleInsertToIndex = async () => {
    if (!result || !result.canonical_hash) return
    try {
      await client.insert({
        doc_id: result.doc_id,
        tenant_id: result.tenant_id,
        canonical_hash: result.canonical_hash,
        perceptual_fingerprint: result.perceptual_fingerprint?.minhash,
        semantic_embedding: result.semantic_embedding?.vector,
      })
      tools.addToast('Inserted into index', 'success')
      tools.addHistory({ type: 'index-insert', success: true, summary: `Inserted ${result.doc_id}` })
      onAfterInsert()
    } catch (err: unknown) {
      tools.addToast(err instanceof Error ? err.message : 'Insert failed', 'error')
    }
  }

  return (
    <SectionWrap>
      <SectionHeader
        icon={<Fingerprint size={18} />}
        title="Process Document"
        desc="Generate perceptual and semantic fingerprints for text content. Pipeline toggles are in the Config panel."
      />

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-zinc-600 dark:text-zinc-400">Input Text</span>
            <span className="text-xs text-zinc-400 dark:text-zinc-600 font-mono">{wordCount} words · {charCount} chars</span>
          </div>
          <textarea
            value={text}
            onChange={e => setText(e.target.value)}
            placeholder="Paste or type the text you want to fingerprint..."
            rows={12}
            className={textareaBase}
          />

          <div className="space-y-3 pt-2">
            <div>
              <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1">Doc ID (optional)</label>
              <input type="text" value={docId} onChange={e => setDocId(e.target.value)} placeholder="Auto-generated UUID" className={inputBase} />
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1">Tenant ID</label>
              <input type="text" value={tenantId} onChange={e => setTenantId(e.target.value)} placeholder={config.defaultTenant || 'default'} className={inputBase} />
            </div>
            <label className="flex items-center gap-3 cursor-pointer">
              <button
                type="button"
                role="switch"
                aria-checked={autoIndex}
                onClick={() => setAutoIndex(!autoIndex)}
                className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors flex-shrink-0 ${
                  autoIndex ? 'bg-accent-600' : 'bg-zinc-300 dark:bg-zinc-700'
                }`}
              >
                <span
                  className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                    autoIndex ? 'translate-x-5' : 'translate-x-1'
                  }`}
                />
              </button>
              <span className="text-sm text-zinc-700 dark:text-zinc-200">Auto-insert into index</span>
            </label>
            <div className="text-[11px] text-zinc-400 dark:text-zinc-600 font-mono">
              perceptual: <strong className={config.enablePerceptual ? 'text-accent-600 dark:text-accent-400' : ''}>{config.enablePerceptual ? 'on' : 'off'}</strong>
              {' · '}semantic: <strong className={config.enableSemantic ? 'text-accent-600 dark:text-accent-400' : ''}>{config.enableSemantic ? 'on' : 'off'}</strong>
            </div>
          </div>

          <button onClick={handleProcess} disabled={loading || !text.trim()} className={`${btnPrimary} w-full`}>
            {loading ? (
              <><span className="inline-block w-3 h-3 border-2 border-white/40 border-t-white rounded-full animate-spin" /> Processing...</>
            ) : (
              <><Fingerprint size={14} /> Fingerprint</>
            )}
          </button>
        </div>

        <div>
          {!result && !loading && (
            <EmptyState
              icon={<Fingerprint size={28} />}
              title="No result yet"
              description="Process text to see fingerprints here"
            />
          )}
          {loading && <LoadingSkeleton lines={5} />}
          {result && (
            <div className="space-y-4">
              <div className="flex items-center justify-between pb-3 border-b border-zinc-200 dark:border-zinc-800">
                <code className="text-xs font-mono text-zinc-600 dark:text-zinc-400 truncate">{result.doc_id}</code>
                <span className={`inline-flex items-center px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded ${
                  result.status === 'success'
                    ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                    : 'bg-rose-500/10 text-rose-600 dark:text-rose-400'
                }`}>
                  {result.status}
                </span>
              </div>

              {result.canonical_hash && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <Hash size={12} className="text-zinc-400" />
                    <span className="text-xs font-medium text-zinc-600 dark:text-zinc-400">Canonical Hash</span>
                    <CopyButton text={result.canonical_hash} />
                  </div>
                  <code className="block px-3 py-2 text-xs font-mono bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-700 dark:text-zinc-300 break-all">
                    {result.canonical_hash}
                  </code>
                </div>
              )}

              {result.perceptual_fingerprint && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <Layers size={12} className="text-zinc-400" />
                    <span className="text-xs font-medium text-zinc-600 dark:text-zinc-400">Perceptual Fingerprint</span>
                  </div>
                  <div className="grid grid-cols-8 gap-1 p-2 rounded-md bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800">
                    {result.perceptual_fingerprint.minhash?.slice(0, 32).map((v: number, i: number) => (
                      <div
                        key={i}
                        title={String(v)}
                        className="aspect-square flex items-center justify-center rounded text-[9px] font-mono text-zinc-700 dark:text-zinc-300"
                        style={{ background: `hsla(${v % 360}, 60%, 50%, 0.2)` }}
                      >
                        {v % 100}
                      </div>
                    ))}
                  </div>
                  <p className="mt-1 text-[11px] text-zinc-400 dark:text-zinc-600 font-mono">
                    {result.perceptual_fingerprint.minhash?.length || 0} hash values
                  </p>
                </div>
              )}

              {result.semantic_embedding && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <BrainIcon size={12} />
                    <span className="text-xs font-medium text-zinc-600 dark:text-zinc-400">Semantic Embedding</span>
                  </div>
                  <div className="flex items-end gap-[2px] h-16 p-2 rounded-md bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800">
                    {result.semantic_embedding.vector?.slice(0, 64).map((v: number, i: number) => (
                      <div
                        key={i}
                        className="flex-1 rounded-sm"
                        style={{
                          height: `${Math.min(100, Math.abs(v) * 100)}%`,
                          background: v >= 0 ? 'var(--color-accent-600)' : '#f43f5e',
                          opacity: 0.4 + Math.min(1, Math.abs(v)) * 0.6,
                        }}
                      />
                    ))}
                  </div>
                  <p className="mt-1 text-[11px] text-zinc-400 dark:text-zinc-600 font-mono">
                    {result.semantic_embedding.vector?.length || 0} dims · {result.semantic_embedding.model_name ?? '—'}
                  </p>
                </div>
              )}

              <div className="flex gap-2 pt-2">
                <button onClick={handleInsertToIndex} className={btnSecondary}>
                  <Plus size={12} /> Add to Index
                </button>
                <button onClick={() => setShowRaw(!showRaw)} className={btnSecondary}>
                  {showRaw ? <EyeOff size={12} /> : <Eye size={12} />}
                  {showRaw ? 'Hide Raw' : 'Show Raw'}
                </button>
              </div>

              {showRaw && (
                <pre className="px-3 py-2 text-[11px] font-mono bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-700 dark:text-zinc-300 overflow-x-auto max-h-64 overflow-y-auto">
                  {JSON.stringify(result, null, 2)}
                </pre>
              )}
            </div>
          )}
        </div>
      </div>
    </SectionWrap>
  )
}

function BrainIcon({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-zinc-400">
      <path d="M12 2a4 4 0 0 1 4 4c0 1.1-.9 2-2 2h-4a2 2 0 0 1-2-2 4 4 0 0 1 4-4z" />
      <path d="M8 8v2a4 4 0 0 0 8 0V8" />
      <path d="M12 14v8" />
      <path d="M8 18h8" />
    </svg>
  )
}

// ─── Index Section ────────────────────────────────────────────────

function IndexSection({ tools }: { tools: Tools }) {
  const client = useClient()
  const [stats, setStats] = useState<{ total: number; perceptual: number; semantic: number } | null>(null)
  const [docs, setDocs] = useState<IndexedDoc[]>([])
  const [loading, setLoading] = useState(false)
  const [showInsert, setShowInsert] = useState(false)
  const [insertForm, setInsertForm] = useState({ doc_id: '', canonical_hash: '', tenant_id: '' })

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const [s, d] = await Promise.all([client.indexStats(), client.listDocuments()])
      setStats({ total: s.total_documents, perceptual: s.with_perceptual, semantic: s.with_semantic })
      setDocs(d.documents)
    } catch {
      tools.addToast('Failed to load index', 'error')
    } finally {
      setLoading(false)
    }
  }, [client, tools])

  useEffect(() => { refresh() }, [refresh])

  const handleDelete = async (docId: string) => {
    if (!confirm(`Delete ${docId}?`)) return
    try {
      await client.deleteDocument(docId)
      tools.addToast(`Deleted ${docId}`, 'success')
      tools.addHistory({ type: 'delete', success: true, summary: `Deleted ${docId}` })
      refresh()
    } catch (err: unknown) {
      tools.addToast(err instanceof Error ? err.message : 'Delete failed', 'error')
    }
  }

  const handleInsert = async () => {
    if (!insertForm.doc_id || !insertForm.canonical_hash) {
      tools.addToast('Doc ID and hash are required', 'error')
      return
    }
    try {
      await client.insert({
        doc_id: insertForm.doc_id,
        tenant_id: insertForm.tenant_id || undefined,
        canonical_hash: insertForm.canonical_hash,
      })
      tools.addToast('Inserted', 'success')
      tools.addHistory({ type: 'index-insert', success: true, summary: `Inserted ${insertForm.doc_id}` })
      setShowInsert(false)
      setInsertForm({ doc_id: '', canonical_hash: '', tenant_id: '' })
      refresh()
    } catch (err: unknown) {
      tools.addToast(err instanceof Error ? err.message : 'Insert failed', 'error')
    }
  }

  return (
    <SectionWrap>
      <SectionHeader
        icon={<Database size={18} />}
        title="Index"
        desc="Manage fingerprint documents in the index."
        right={
          <div className="flex items-center gap-2">
            <button onClick={() => setShowInsert(!showInsert)} className={btnSecondary}>
              <Plus size={12} /> Insert
            </button>
            <button onClick={refresh} className={btnSecondary}>
              <RefreshCw size={12} /> Refresh
            </button>
          </div>
        }
      />

      {stats && (
        <div className="grid grid-cols-3 gap-3 mb-5">
          {[
            { label: 'Total Documents', value: stats.total },
            { label: 'With Perceptual', value: stats.perceptual },
            { label: 'With Semantic', value: stats.semantic },
          ].map(s => (
            <div key={s.label} className="px-4 py-3 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-950/50">
              <div className="text-2xl font-semibold text-zinc-900 dark:text-zinc-50">{s.value}</div>
              <div className="mt-0.5 text-[11px] uppercase tracking-wider text-zinc-500 dark:text-zinc-500">{s.label}</div>
            </div>
          ))}
        </div>
      )}

      <AnimatePresence>
        {showInsert && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden mb-5"
          >
            <div className="p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-950/50 space-y-3">
              <h4 className="text-sm font-semibold text-zinc-900 dark:text-zinc-50">Insert Record</h4>
              <div className="grid grid-cols-2 gap-2">
                <input placeholder="Doc ID *" value={insertForm.doc_id} onChange={e => setInsertForm(p => ({ ...p, doc_id: e.target.value }))} className={inputBase} />
                <input placeholder="Tenant ID" value={insertForm.tenant_id} onChange={e => setInsertForm(p => ({ ...p, tenant_id: e.target.value }))} className={inputBase} />
              </div>
              <input placeholder="Canonical Hash *" value={insertForm.canonical_hash} onChange={e => setInsertForm(p => ({ ...p, canonical_hash: e.target.value }))} className={inputBase} />
              <div className="flex gap-2">
                <button onClick={handleInsert} className={btnPrimary}>Insert</button>
                <button onClick={() => setShowInsert(false)} className={btnSecondary}>Cancel</button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {loading && <LoadingSkeleton lines={4} />}
      {!loading && docs.length === 0 && (
        <EmptyState icon={<Database size={28} />} title="Index is empty" description="Process documents and insert them here" />
      )}
      {!loading && docs.length > 0 && (
        <div className="space-y-2">
          {docs.map(doc => (
            <div
              key={doc.doc_id}
              className="flex items-center justify-between gap-3 px-4 py-3 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <div className="flex items-center gap-3 min-w-0">
                <code className="text-sm font-mono text-zinc-900 dark:text-zinc-50 truncate">{doc.doc_id}</code>
                <code className="text-xs font-mono text-zinc-400 dark:text-zinc-600 truncate">{doc.canonical_hash?.slice(0, 16)}…</code>
                {doc.tenant_id && (
                  <span className="inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400">
                    {doc.tenant_id}
                  </span>
                )}
              </div>
              <button
                onClick={() => handleDelete(doc.doc_id)}
                title="Delete"
                className="p-1.5 rounded-md text-zinc-400 hover:text-rose-500 hover:bg-rose-500/10 transition-colors"
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
        </div>
      )}
    </SectionWrap>
  )
}

// ─── Search Section ────────────────────────────────────────────────

function SearchSection({ tools }: { tools: Tools }) {
  const client = useClient()
  const { config, updateMatch } = useConfig()
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(false)
  const [results, setResults] = useState<(SearchHit | MatchHit)[]>([])

  const handleSearch = async () => {
    if (!query.trim()) { tools.addToast('Enter search query', 'error'); return }
    setLoading(true)
    setResults([])
    try {
      const res = await client.match({
        query,
        strategy: config.match.strategy,
        max_results: config.match.max_results,
        min_score: config.match.min_score,
        tenant_id: config.defaultTenant || undefined,
      })
      setResults(res.matches)
      tools.addToast(`Found ${res.total_matches} matches`, 'success')
      tools.addHistory({ type: 'search', success: true, summary: `"${query.slice(0, 40)}" → ${res.total_matches} results` })
    } catch (err: unknown) {
      tools.addToast(err instanceof Error ? err.message : 'Search failed', 'error')
    } finally {
      setLoading(false)
    }
  }

  return (
    <SectionWrap>
      <SectionHeader
        icon={<Search size={18} />}
        title="Search & Match"
        desc="Find similar documents using perceptual, semantic, or hybrid strategies."
      />

      <div className="space-y-3 mb-5">
        <input
          value={query}
          onChange={e => setQuery(e.target.value)}
          placeholder="Enter search query text..."
          onKeyDown={e => e.key === 'Enter' && handleSearch()}
          className={inputBase}
        />
        <div className="flex items-center gap-3 flex-wrap">
          <select
            value={config.match.strategy}
            onChange={e => updateMatch('strategy', e.target.value as 'perceptual' | 'semantic' | 'hybrid')}
            className={`${inputBase} max-w-[150px]`}
          >
            <option value="hybrid">Hybrid</option>
            <option value="perceptual">Perceptual</option>
            <option value="semantic">Semantic</option>
          </select>
          <label className="flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-400">
            <span>Top K: {config.match.max_results}</span>
            <input
              type="range"
              min={1}
              max={50}
              value={config.match.max_results}
              onChange={e => updateMatch('max_results', Number(e.target.value))}
              className="w-32 accent-accent-600"
            />
          </label>
          <button onClick={handleSearch} disabled={loading || !query.trim()} className={`${btnPrimary} ml-auto`}>
            {loading ? (
              <><span className="inline-block w-3 h-3 border-2 border-white/40 border-t-white rounded-full animate-spin" /> Searching...</>
            ) : (
              <><Search size={14} /> Search</>
            )}
          </button>
        </div>
      </div>

      {loading && <LoadingSkeleton lines={4} />}
      {!loading && results.length === 0 && query && (
        <EmptyState icon={<Search size={28} />} title="No matches found" description="Try a different query or strategy" />
      )}
      {!loading && results.length > 0 && (
        <div className="space-y-2">
          {results.map((hit, i) => {
            const scoreColor =
              hit.score >= 0.7 ? 'text-emerald-500'
              : hit.score >= 0.4 ? 'text-amber-500'
              : 'text-rose-500'
            const barColor =
              hit.score >= 0.7 ? 'bg-emerald-500'
              : hit.score >= 0.4 ? 'bg-amber-500'
              : 'bg-rose-500'

            return (
              <motion.div
                key={hit.doc_id + i}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.04 }}
                className="px-4 py-3 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900"
              >
                <div className="flex items-center justify-between gap-3 mb-2">
                  <div className="flex items-center gap-3 min-w-0">
                    <span className="text-xs font-mono text-zinc-400 dark:text-zinc-600">#{i + 1}</span>
                    <code className="text-sm font-mono text-zinc-900 dark:text-zinc-50 truncate">{hit.doc_id}</code>
                  </div>
                  <span className={`text-sm font-semibold ${scoreColor}`}>
                    {(hit.score * 100).toFixed(1)}%
                  </span>
                </div>
                <div className="h-1 rounded-full bg-zinc-100 dark:bg-zinc-800 overflow-hidden">
                  <div className={`h-full ${barColor} transition-all`} style={{ width: `${hit.score * 100}%` }} />
                </div>
                {hit.tenant_id && (
                  <span className="inline-flex mt-2 items-center px-1.5 py-0.5 text-[10px] font-medium rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400">
                    {hit.tenant_id}
                  </span>
                )}
              </motion.div>
            )
          })}
        </div>
      )}
    </SectionWrap>
  )
}

// ─── Compare Section ────────────────────────────────────────────────

function CompareSection({ tools }: { tools: Tools }) {
  const client = useClient()
  const [doc1, setDoc1] = useState('')
  const [doc2, setDoc2] = useState('')
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<{ similarity_score: number; perceptual_similarity: number | null; semantic_similarity: number | null } | null>(null)

  const handleCompare = async () => {
    if (!doc1.trim() || !doc2.trim()) { tools.addToast('Enter both documents', 'error'); return }
    setLoading(true)
    setResult(null)
    try {
      const res = await client.compare({ text: doc1 }, { text: doc2 })
      setResult(res)
      tools.addToast(`Similarity: ${(res.similarity_score * 100).toFixed(1)}%`, 'success')
      tools.addHistory({ type: 'compare', success: true, summary: `${(res.similarity_score * 100).toFixed(1)}% match` })
    } catch (err: unknown) {
      tools.addToast(err instanceof Error ? err.message : 'Compare failed', 'error')
    } finally {
      setLoading(false)
    }
  }

  return (
    <SectionWrap>
      <SectionHeader
        icon={<GitCompare size={18} />}
        title="Compare Documents"
        desc="Compare two texts for perceptual and semantic similarity."
      />

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-5">
        <div>
          <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1.5">Document 1</label>
          <textarea value={doc1} onChange={e => setDoc1(e.target.value)} placeholder="First document text..." rows={8} className={textareaBase} />
        </div>
        <div>
          <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1.5">Document 2</label>
          <textarea value={doc2} onChange={e => setDoc2(e.target.value)} placeholder="Second document text..." rows={8} className={textareaBase} />
        </div>
      </div>

      <div className="flex justify-center mb-5">
        <button onClick={handleCompare} disabled={loading || !doc1.trim() || !doc2.trim()} className={btnPrimary}>
          {loading ? (
            <><span className="inline-block w-3 h-3 border-2 border-white/40 border-t-white rounded-full animate-spin" /> Comparing...</>
          ) : (
            <><GitCompare size={14} /> Compare</>
          )}
        </button>
      </div>

      {result && (
        <div className="flex items-center justify-center gap-8 py-6 border-t border-zinc-200 dark:border-zinc-800">
          <ScoreGauge value={result.similarity_score} label="Combined" size={100} />
          {result.perceptual_similarity !== null && (
            <ScoreGauge value={result.perceptual_similarity} label="Perceptual" />
          )}
          {result.semantic_similarity !== null && (
            <ScoreGauge value={result.semantic_similarity} label="Semantic" />
          )}
        </div>
      )}
    </SectionWrap>
  )
}

// ─── History Section ────────────────────────────────────────────────

function HistorySection({ history, setHistory }: { history: HistoryEntry[]; setHistory: (h: HistoryEntry[]) => void }) {
  const [expanded, setExpanded] = useState<string | null>(null)

  const timeAgo = (ts: number) => {
    const s = Math.floor((Date.now() - ts) / 1000)
    if (s < 60) return `${s}s ago`
    if (s < 3600) return `${Math.floor(s / 60)}m ago`
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`
    return `${Math.floor(s / 86400)}d ago`
  }

  const typeIcon = (type: string) => {
    switch (type) {
      case 'process': return <Fingerprint size={12} />
      case 'index-insert': return <Plus size={12} />
      case 'search': return <Search size={12} />
      case 'compare': return <GitCompare size={12} />
      case 'delete': return <Trash2 size={12} />
      default: return <Info size={12} />
    }
  }

  return (
    <SectionWrap>
      <SectionHeader
        icon={<Clock size={18} />}
        title="History"
        desc={`Recent operations (${history.length})`}
        right={history.length > 0 ? (
          <button onClick={() => setHistory([])} className={btnSecondary}>Clear</button>
        ) : undefined}
      />

      {history.length === 0 && (
        <EmptyState icon={<Clock size={28} />} title="No history" description="Operations will appear here" />
      )}

      <div className="space-y-2">
        {history.map(entry => (
          <div
            key={entry.id}
            className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 overflow-hidden"
          >
            <button
              onClick={() => setExpanded(expanded === entry.id ? null : entry.id)}
              className="w-full flex items-center gap-3 px-4 py-3 hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors"
            >
              <span className={entry.success ? 'text-emerald-500' : 'text-rose-500'}>
                {entry.success ? <CheckCircle size={14} /> : <AlertCircle size={14} />}
              </span>
              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400">
                {typeIcon(entry.type)} {entry.type}
              </span>
              <span className="flex-1 text-left text-sm text-zinc-700 dark:text-zinc-300 truncate">{entry.summary}</span>
              <span className="text-xs text-zinc-400 dark:text-zinc-600 font-mono">{timeAgo(entry.timestamp)}</span>
              {expanded === entry.id ? <ChevronUp size={12} className="text-zinc-400" /> : <ChevronDown size={12} className="text-zinc-400" />}
            </button>
            {expanded === entry.id && entry.data != null && (
              <pre className="px-4 py-3 text-[11px] font-mono bg-zinc-50 dark:bg-zinc-950 border-t border-zinc-200 dark:border-zinc-800 text-zinc-700 dark:text-zinc-300 overflow-x-auto max-h-64 overflow-y-auto">
                {JSON.stringify(entry.data, null, 2)}
              </pre>
            )}
          </div>
        ))}
      </div>
    </SectionWrap>
  )
}
