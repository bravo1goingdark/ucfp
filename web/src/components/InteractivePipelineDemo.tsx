import { useEffect, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Inbox, TextQuote, Fingerprint, Brain, Database, Search,
  ArrowRight, Check, Loader2, AlertCircle, WifiOff,
} from 'lucide-react'
import { useClient, useProcessRequestBuilder } from '../api/useClient'
import { useConfig } from '../context/ConfigContext'

const K_MIN = 1
const K_MAX = 32

type StageState = 'idle' | 'processing' | 'done' | 'skipped' | 'error'

interface StageData {
  icon: typeof Inbox
  name: string
  hint: string
}

const stages: StageData[] = [
  { icon: Inbox, name: 'Ingest', hint: 'Validate + metadata' },
  { icon: TextQuote, name: 'Canonical', hint: 'NFKC + SHA-256' },
  { icon: Fingerprint, name: 'Perceptual', hint: 'MinHash LSH' },
  { icon: Brain, name: 'Semantic', hint: 'Embedding' },
  { icon: Database, name: 'Index', hint: 'Upsert' },
  { icon: Search, name: 'Match', hint: 'Similarity' },
]

function fmtMs(ms: number): string {
  if (ms < 1) return `${(ms * 1000).toFixed(0)}μs`
  if (ms < 1000) return `${ms.toFixed(0)}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

export default function InteractivePipelineDemo() {
  const client = useClient()
  const buildRequest = useProcessRequestBuilder()
  const { config, updatePerceptual } = useConfig()

  const [input, setInput] = useState('The quick brown fox jumps over the lazy dog')
  const [states, setStates] = useState<StageState[]>(() => Array(stages.length).fill('idle'))
  const [outputs, setOutputs] = useState<string[]>(() => Array(stages.length).fill(''))
  const [isRunning, setIsRunning] = useState(false)
  const [serverOnline, setServerOnline] = useState<boolean | null>(null)
  const [totalTime, setTotalTime] = useState<number | null>(null)
  const [canonicalHash, setCanonicalHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const [isVisible, setIsVisible] = useState(false)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) setIsVisible(true) },
      { threshold: 0.2 },
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    let cancelled = false
    client.testConnection().then(ok => { if (!cancelled) setServerOnline(ok) })
    return () => { cancelled = true }
  }, [client])

  const delay = (ms: number) => new Promise(r => setTimeout(r, ms))

  const runPipeline = async () => {
    if (isRunning || !input.trim()) return
    setIsRunning(true)
    setError(null)
    setCanonicalHash(null)
    setTotalTime(null)
    setStates(Array(stages.length).fill('idle'))
    setOutputs(Array(stages.length).fill(''))

    const start = performance.now()

    const markState = (i: number, s: StageState) =>
      setStates(prev => { const n = [...prev]; n[i] = s; return n })
    const markOutput = (i: number, text: string) =>
      setOutputs(prev => { const n = [...prev]; n[i] = text; return n })

    markState(0, 'processing')
    markState(1, 'processing')
    if (config.enablePerceptual) markState(2, 'processing'); else markState(2, 'skipped')
    if (config.enableSemantic) markState(3, 'processing'); else markState(3, 'skipped')

    try {
      const req = buildRequest(input)
      const res = await client.process(req)

      const elapsed = performance.now() - start
      setTotalTime(elapsed)

      markState(0, 'done')
      markOutput(0, `${input.length} chars · tenant=${res.tenant_id}`)

      if (res.status !== 'success') {
        setError(res.error || 'Pipeline failed')
        markState(1, 'error')
        setIsRunning(false)
        return
      }

      markState(1, 'done')
      if (res.canonical_hash) {
        markOutput(1, `SHA-256: ${res.canonical_hash.slice(0, 16)}…`)
        setCanonicalHash(res.canonical_hash)
      } else {
        markOutput(1, 'canonicalized')
      }

      if (config.enablePerceptual) {
        markState(2, 'done')
        const minhash = res.perceptual_fingerprint?.minhash
        if (minhash?.length) {
          markOutput(2, `MinHash · ${minhash.length} values · first=[${minhash.slice(0, 4).join(', ')}…]`)
        } else {
          markOutput(2, 'no fingerprint returned')
        }
      }

      if (config.enableSemantic) {
        markState(3, 'done')
        const vector = res.semantic_embedding?.vector
        if (vector?.length) {
          markOutput(3, `Embedding · ${vector.length} dims · ${res.semantic_embedding?.model_name ?? 'model'}`)
        } else {
          markOutput(3, 'no embedding returned')
        }
      }

      markState(4, 'processing')
      await delay(120)
      try {
        if (res.canonical_hash) {
          await client.insert({
            doc_id: res.doc_id,
            tenant_id: res.tenant_id,
            canonical_hash: res.canonical_hash,
            perceptual_fingerprint: res.perceptual_fingerprint?.minhash,
            semantic_embedding: res.semantic_embedding?.vector,
          })
          markState(4, 'done')
          markOutput(4, `Upserted ${res.doc_id.slice(0, 8)}`)
        } else {
          markState(4, 'skipped')
          markOutput(4, 'skipped (no hash)')
        }
      } catch (e) {
        markState(4, 'error')
        markOutput(4, `index error: ${(e as Error).message}`)
      }

      markState(5, 'processing')
      try {
        const m = await client.match({
          query: input,
          strategy: config.match.strategy,
          max_results: config.match.max_results,
          min_score: config.match.min_score,
          tenant_id: config.defaultTenant || undefined,
        })
        markState(5, 'done')
        markOutput(5, `${m.total_matches} matches · strategy=${m.strategy}`)
      } catch (e) {
        markState(5, 'error')
        markOutput(5, `match error: ${(e as Error).message}`)
      }
    } catch (e) {
      const msg = (e as Error).message || 'Unknown error'
      setError(msg)
      setStates(prev => prev.map(s => (s === 'processing' ? 'error' : s)))
      setServerOnline(false)
    } finally {
      setIsRunning(false)
    }
  }

  const offline = serverOnline === false

  const stateStyles = (state: StageState) => {
    if (state === 'processing') return 'border-accent-500/40 bg-accent-500/5'
    if (state === 'done') return 'border-emerald-500/30 bg-emerald-500/5'
    if (state === 'error') return 'border-rose-500/40 bg-rose-500/5'
    if (state === 'skipped') return 'border-zinc-200/50 dark:border-zinc-800/50 opacity-50'
    return 'border-zinc-200 dark:border-zinc-800'
  }

  const iconStyles = (state: StageState) => {
    if (state === 'processing') return 'bg-accent-500/10 text-accent-600 dark:text-accent-400'
    if (state === 'done') return 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
    if (state === 'error') return 'bg-rose-500/10 text-rose-600 dark:text-rose-400'
    return 'bg-zinc-100 dark:bg-zinc-800 text-zinc-400 dark:text-zinc-500'
  }

  return (
    <div ref={containerRef} className="my-10">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={isVisible ? { opacity: 1, y: 0 } : {}}
        transition={{ duration: 0.5 }}
        className="rounded-2xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-6 sm:p-8"
      >
        {/* Status bar */}
        <div className="flex items-center justify-between flex-wrap gap-3 mb-5 text-xs">
          <div className="flex items-center gap-2 text-zinc-500 dark:text-zinc-400">
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                serverOnline == null ? 'bg-zinc-400'
                : serverOnline ? 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)]'
                : 'bg-rose-500'
              }`}
            />
            <code className="font-mono">{config.serverUrl}</code>
            <span className="opacity-50">·</span>
            <span>{serverOnline == null ? 'checking…' : serverOnline ? 'live backend' : 'offline'}</span>
          </div>
          <div className="flex items-center gap-3 text-zinc-400 dark:text-zinc-500 font-mono">
            <span>p=<strong className={config.enablePerceptual ? 'text-accent-600 dark:text-accent-400' : ''}>{config.enablePerceptual ? 'on' : 'off'}</strong></span>
            <span>s=<strong className={config.enableSemantic ? 'text-accent-600 dark:text-accent-400' : ''}>{config.enableSemantic ? 'on' : 'off'}</strong></span>
          </div>
        </div>

        {offline && (
          <div className="flex items-center gap-2 px-3 py-2 mb-4 rounded-md border border-rose-500/20 bg-rose-500/5 text-xs text-rose-600 dark:text-rose-400">
            <WifiOff size={14} />
            Backend unreachable. Start the server with <code className="font-mono px-1">cargo run -p server</code>.
          </div>
        )}

        {/* Input */}
        <div className="mb-5">
          <label className="block text-[10px] uppercase tracking-wider font-semibold text-zinc-500 dark:text-zinc-500 mb-2">
            Input Text
          </label>
          <div className="flex flex-col sm:flex-row gap-2">
            <input
              type="text"
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && runPipeline()}
              placeholder="Enter text to fingerprint..."
              disabled={isRunning}
              className="flex-1 px-4 py-3 text-sm font-mono bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-lg text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500 disabled:opacity-60"
            />
            <button
              onClick={runPipeline}
              disabled={isRunning || !input.trim()}
              className="inline-flex items-center justify-center gap-2 px-6 py-3 text-sm font-medium rounded-lg bg-accent-600 hover:bg-accent-700 text-white disabled:opacity-60 disabled:cursor-not-allowed transition-colors whitespace-nowrap"
            >
              {isRunning ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
              {isRunning ? 'Processing…' : 'Run Pipeline'}
            </button>
          </div>

          {/* Inline perceptual tuning */}
          <div className="mt-3 flex flex-wrap items-center gap-3 px-3 py-2 rounded-lg border border-dashed border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-950/50">
            <label className="flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-400">
              <Fingerprint size={12} className="text-accent-600 dark:text-accent-400" />
              <span className="font-medium">Shingle size</span>
              <code className="font-mono text-zinc-900 dark:text-zinc-50">k = {config.perceptual.k}</code>
            </label>
            <input
              type="range"
              min={K_MIN}
              max={K_MAX}
              value={config.perceptual.k}
              onChange={e => updatePerceptual('k', Number(e.target.value))}
              disabled={isRunning || !config.enablePerceptual}
              className="flex-1 min-w-[120px] max-w-[260px] accent-accent-600 disabled:opacity-40"
            />
            <input
              type="number"
              min={K_MIN}
              max={K_MAX}
              value={config.perceptual.k}
              onChange={e => {
                const v = Number(e.target.value)
                if (!Number.isNaN(v)) updatePerceptual('k', Math.max(K_MIN, Math.min(K_MAX, v)))
              }}
              disabled={isRunning || !config.enablePerceptual}
              className="w-16 px-2 py-1 text-xs font-mono bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded text-zinc-900 dark:text-zinc-50 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500 disabled:opacity-40"
            />
            <span className="text-[10px] text-zinc-400 dark:text-zinc-600">
              {config.enablePerceptual ? 'tokens per shingle — larger = more context' : 'enable perceptual to apply'}
            </span>
          </div>
        </div>

        {/* Stages */}
        <div className="space-y-1.5">
          {stages.map((stage, i) => {
            const state = states[i]
            const output = outputs[i]
            const Icon = stage.icon

            return (
              <motion.div
                key={stage.name}
                initial={{ opacity: 0, x: -10 }}
                animate={isVisible ? { opacity: 1, x: 0 } : {}}
                transition={{ delay: i * 0.04, duration: 0.3 }}
                className={`flex items-center gap-3 px-3 py-2.5 rounded-lg border transition-all ${stateStyles(state)}`}
              >
                <div
                  className={`flex items-center justify-center w-8 h-8 rounded-md flex-shrink-0 transition-colors ${iconStyles(state)}`}
                >
                  {state === 'processing' ? <Loader2 size={14} className="animate-spin" />
                    : state === 'done' ? <Check size={14} />
                    : state === 'error' ? <AlertCircle size={14} />
                    : <Icon size={14} strokeWidth={1.75} />}
                </div>
                <span
                  className={`text-sm font-medium min-w-[90px] ${
                    state === 'idle' || state === 'skipped'
                      ? 'text-zinc-400 dark:text-zinc-600'
                      : 'text-zinc-900 dark:text-zinc-100'
                  }`}
                >
                  {stage.name}
                </span>
                {output ? (
                  <code className="flex-1 text-xs font-mono text-zinc-600 dark:text-zinc-400 truncate">
                    {output}
                  </code>
                ) : (
                  <span className="flex-1 text-xs font-mono text-zinc-400 dark:text-zinc-600 truncate">
                    {stage.hint}
                  </span>
                )}
              </motion.div>
            )
          })}
        </div>

        {/* Total time + hash */}
        <AnimatePresence>
          {(canonicalHash || totalTime != null) && (
            <motion.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="mt-5 px-4 py-3 rounded-lg border border-emerald-500/20 bg-emerald-500/5"
            >
              <div className="flex items-center justify-between">
                <span className="text-[10px] uppercase tracking-wider font-semibold text-emerald-600 dark:text-emerald-400">
                  {canonicalHash ? 'Canonical Hash' : 'Result'}
                </span>
                {totalTime != null && (
                  <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
                    total · {fmtMs(totalTime)}
                  </span>
                )}
              </div>
              {canonicalHash && (
                <code className="block mt-2 text-xs font-mono text-zinc-700 dark:text-zinc-300 break-all">
                  {canonicalHash}
                </code>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {error && !offline && (
          <div className="mt-4 flex items-center gap-2 px-3 py-2 rounded-md border border-rose-500/20 bg-rose-500/5 text-xs text-rose-600 dark:text-rose-400">
            <AlertCircle size={14} />
            {error}
          </div>
        )}
      </motion.div>
    </div>
  )
}
