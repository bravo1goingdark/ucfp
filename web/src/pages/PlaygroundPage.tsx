import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Terminal, Copy, Check, Settings, Server,
  Search, GitCompare, Layers, Zap, RefreshCw,
  ChevronDown, ChevronUp, AlertCircle, Clock, Activity, ArrowRight,
  Save, FolderOpen, Send, BarChart3, Code2, Sliders,
} from 'lucide-react'
import { ScrollProgress } from '../components/ScrollAnimations'
import ConfigPanel from '../components/ConfigPanel'
import { useConfig } from '../context/ConfigContext'

interface EndpointDef {
  id: string
  label: string
  method: 'GET' | 'POST' | 'DELETE'
  path: string
  icon: typeof Terminal
  category: 'public' | 'process' | 'index' | 'match' | 'system'
  description: string
  requiresAuth: boolean
  defaultBody?: Record<string, unknown>
  queryParams?: { key: string; placeholder: string; type: string }[]
}

const endpoints: EndpointDef[] = [
  { id: 'health', label: 'Health Check', method: 'GET', path: '/health', icon: Activity, category: 'public', requiresAuth: false, description: 'Liveness probe — returns 200 if server is running' },
  { id: 'ready', label: 'Readiness', method: 'GET', path: '/ready', icon: Activity, category: 'public', requiresAuth: false, description: 'Readiness probe — checks all components' },
  { id: 'metrics', label: 'Metrics', method: 'GET', path: '/metrics', icon: BarChart3, category: 'public', requiresAuth: false, description: 'Prometheus-style metrics and index stats' },
  { id: 'root', label: 'API Info', method: 'GET', path: '/', icon: Server, category: 'public', requiresAuth: false, description: 'Returns API information and available endpoints' },
  {
    id: 'process', label: 'Process Document', method: 'POST', path: '/api/v1/process', icon: Terminal,
    category: 'process', requiresAuth: true,
    description: 'Process a single document through the fingerprinting pipeline',
    defaultBody: { doc_id: 'demo-001', text: 'The quick brown fox jumps over the lazy dog.', enable_perceptual: true, enable_semantic: false },
  },
  {
    id: 'batch', label: 'Batch Process', method: 'POST', path: '/api/v1/batch', icon: Layers,
    category: 'process', requiresAuth: true,
    description: 'Process multiple documents in a single request',
    defaultBody: {
      documents: [
        { doc_id: 'doc-001', text: 'First document about Rust programming' },
        { doc_id: 'doc-002', text: 'Second document about memory safety' },
      ],
      enable_perceptual: true, enable_semantic: false,
    },
  },
  {
    id: 'index-insert', label: 'Insert to Index', method: 'POST', path: '/api/v1/index/insert', icon: Save,
    category: 'index', requiresAuth: true,
    description: 'Insert a processed record into the index for searching',
    defaultBody: {
      doc_id: 'doc-001',
      canonical_hash: 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4',
      perceptual_fingerprint: [12345, 67890, 11111, 22222],
      semantic_embedding: [0.1, 0.2, -0.3, 0.4],
    },
  },
  {
    id: 'index-search', label: 'Search Index', method: 'GET', path: '/api/v1/index/search', icon: Search,
    category: 'index', requiresAuth: true,
    description: 'Search the index for similar documents',
    queryParams: [
      { key: 'query', placeholder: 'Search query text', type: 'text' },
      { key: 'strategy', placeholder: 'perceptual', type: 'text' },
      { key: 'top_k', placeholder: '10', type: 'number' },
    ],
  },
  { id: 'index-stats', label: 'Index Stats', method: 'GET', path: '/api/v1/index/stats', icon: BarChart3, category: 'index', requiresAuth: true, description: 'Get index statistics and document counts' },
  { id: 'index-docs', label: 'List Documents', method: 'GET', path: '/api/v1/index/documents', icon: FolderOpen, category: 'index', requiresAuth: true, description: 'List all documents in the index' },
  {
    id: 'match', label: 'Match Query', method: 'POST', path: '/api/v1/match', icon: GitCompare,
    category: 'match', requiresAuth: true,
    description: 'Find documents matching a query using similarity',
    defaultBody: { query: 'Rust programming language', strategy: 'hybrid', max_results: 10, min_score: 0.5 },
  },
  {
    id: 'compare', label: 'Compare Documents', method: 'POST', path: '/api/v1/compare', icon: GitCompare,
    category: 'match', requiresAuth: true,
    description: 'Compare two documents directly for similarity',
    defaultBody: { doc1: { text: 'The quick brown fox', doc_id: 'doc-a' }, doc2: { text: 'The quick brown dog', doc_id: 'doc-b' } },
  },
  { id: 'pipeline-status', label: 'Pipeline Status', method: 'GET', path: '/api/v1/pipeline/status', icon: Zap, category: 'system', requiresAuth: true, description: 'Get pipeline and component status' },
]

const categories = [
  { id: 'all', label: 'All' },
  { id: 'public', label: 'Public' },
  { id: 'process', label: 'Process' },
  { id: 'index', label: 'Index' },
  { id: 'match', label: 'Match' },
  { id: 'system', label: 'System' },
] as const

type SnippetLang = 'curl' | 'js' | 'python'

function buildSnippet(
  lang: SnippetLang,
  endpoint: EndpointDef,
  serverUrl: string,
  apiKey: string,
  body: string,
  queryString: string,
): string {
  const fullPath = `${endpoint.path}${queryString ? `?${queryString}` : ''}`
  const fullUrl = `${serverUrl}${fullPath}`

  if (lang === 'curl') {
    const parts = [`curl -X ${endpoint.method} '${fullUrl}'`]
    parts.push(`  -H 'Content-Type: application/json'`)
    if (endpoint.requiresAuth && apiKey) parts.push(`  -H 'X-API-Key: ${apiKey}'`)
    if (endpoint.method === 'POST' && body.trim()) {
      parts.push(`  -d '${body.replace(/'/g, "'\\''")}'`)
    }
    return parts.join(' \\\n')
  }

  if (lang === 'js') {
    const headers: string[] = [`  'Content-Type': 'application/json',`]
    if (endpoint.requiresAuth && apiKey) headers.push(`  'X-API-Key': '${apiKey}',`)
    const lines = [
      `const res = await fetch('${fullUrl}', {`,
      `  method: '${endpoint.method}',`,
      `  headers: {`,
      ...headers,
      `  },`,
    ]
    if (endpoint.method === 'POST' && body.trim()) {
      lines.push(`  body: JSON.stringify(${body.trim()}),`)
    }
    lines.push(`})`)
    lines.push(`const data = await res.json()`)
    lines.push(`console.log(data)`)
    return lines.join('\n')
  }

  const hdrs: string[] = [`    "Content-Type": "application/json",`]
  if (endpoint.requiresAuth && apiKey) hdrs.push(`    "X-API-Key": "${apiKey}",`)
  const lines = [`import requests`, ``]
  if (endpoint.method === 'POST' && body.trim()) {
    lines.push(`payload = ${body.trim()}`)
  }
  lines.push(`headers = {`, ...hdrs, `}`)
  if (endpoint.method === 'POST' && body.trim()) {
    lines.push(`res = requests.${endpoint.method.toLowerCase()}("${fullUrl}", json=payload, headers=headers)`)
  } else {
    lines.push(`res = requests.${endpoint.method.toLowerCase()}("${fullUrl}", headers=headers)`)
  }
  lines.push(`print(res.json())`)
  return lines.join('\n')
}

const methodClass = (method: string): string => {
  switch (method) {
    case 'GET': return 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
    case 'POST': return 'bg-sky-500/10 text-sky-600 dark:text-sky-400'
    case 'DELETE': return 'bg-rose-500/10 text-rose-600 dark:text-rose-400'
    default: return 'bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400'
  }
}

const statusClass = (status: number): string => {
  if (status >= 200 && status < 300) return 'text-emerald-600 dark:text-emerald-400'
  if (status >= 400 && status < 500) return 'text-amber-600 dark:text-amber-400'
  if (status >= 500) return 'text-rose-600 dark:text-rose-400'
  return 'text-zinc-500'
}

export default function PlaygroundPage() {
  const { config } = useConfig()
  const [selectedEndpoint, setSelectedEndpoint] = useState<EndpointDef>(endpoints[4])
  const [requestBody, setRequestBody] = useState('')
  const [queryParams, setQueryParams] = useState<Record<string, string>>({})
  const [response, setResponse] = useState<{ status: number; headers: Record<string, string>; body: unknown } | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [activeCategory, setActiveCategory] = useState<typeof categories[number]['id']>('all')
  const [history, setHistory] = useState<Array<{ endpoint: string; method: string; status: number; time: number; timestamp: number }>>([])
  const [showHistory, setShowHistory] = useState(false)
  const [copied, setCopied] = useState(false)
  const [showConfig, setShowConfig] = useState(false)
  const [responseTime, setResponseTime] = useState<number | null>(null)
  const [snippetLang, setSnippetLang] = useState<SnippetLang>('curl')
  const [snippetCopied, setSnippetCopied] = useState(false)
  const responseRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const saved = localStorage.getItem('ucfp-endpoint')
    if (saved) {
      const found = endpoints.find(e => e.id === saved)
      if (found) {
        setSelectedEndpoint(found)
        setRequestBody(found.defaultBody ? JSON.stringify(found.defaultBody, null, 2) : '')
        return
      }
    }
    setRequestBody(JSON.stringify(endpoints[4].defaultBody || {}, null, 2))
  }, [])

  const handleEndpointChange = useCallback((ep: EndpointDef) => {
    setSelectedEndpoint(ep)
    setRequestBody(ep.defaultBody ? JSON.stringify(ep.defaultBody, null, 2) : '')
    setQueryParams({})
    setResponse(null)
    setError(null)
    setResponseTime(null)
    localStorage.setItem('ucfp-endpoint', ep.id)
  }, [])

  const queryString = useMemo(() => {
    if (!selectedEndpoint.queryParams) return ''
    const params = new URLSearchParams()
    for (const qp of selectedEndpoint.queryParams) {
      if (queryParams[qp.key]) params.append(qp.key, queryParams[qp.key])
    }
    return params.toString()
  }, [selectedEndpoint, queryParams])

  const executeRequest = async () => {
    setIsLoading(true)
    setError(null)
    setResponse(null)
    setResponseTime(null)

    const startTime = performance.now()
    const url = `${config.serverUrl.replace(/\/$/, '')}${selectedEndpoint.path}${queryString ? `?${queryString}` : ''}`

    const headers: Record<string, string> = { 'Content-Type': 'application/json' }
    if (selectedEndpoint.requiresAuth && config.apiKey) {
      headers['X-API-Key'] = config.apiKey
    }

    try {
      const fetchOptions: RequestInit = {
        method: selectedEndpoint.method,
        headers,
      }
      if (selectedEndpoint.method === 'POST' && requestBody) {
        fetchOptions.body = requestBody
      }

      const res = await fetch(url, fetchOptions)
      const elapsed = Math.round(performance.now() - startTime)

      const responseHeaders: Record<string, string> = {}
      res.headers.forEach((value, key) => { responseHeaders[key] = value })

      let body: unknown
      const contentType = res.headers.get('content-type') || ''
      if (contentType.includes('application/json')) {
        body = await res.json()
      } else {
        body = await res.text()
      }

      setResponse({ status: res.status, headers: responseHeaders, body })
      setResponseTime(elapsed)

      setHistory(prev => [{
        endpoint: selectedEndpoint.path,
        method: selectedEndpoint.method,
        status: res.status,
        time: elapsed,
        timestamp: Date.now(),
      }, ...prev].slice(0, 50))
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Unknown error')
      setResponseTime(Math.round(performance.now() - startTime))
    } finally {
      setIsLoading(false)
    }
  }

  const handleCopyResponse = () => {
    if (response) {
      navigator.clipboard.writeText(JSON.stringify(response.body, null, 2))
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const snippet = useMemo(
    () => buildSnippet(snippetLang, selectedEndpoint, config.serverUrl, config.apiKey, requestBody, queryString),
    [snippetLang, selectedEndpoint, config.serverUrl, config.apiKey, requestBody, queryString],
  )

  const handleCopySnippet = () => {
    navigator.clipboard.writeText(snippet)
    setSnippetCopied(true)
    setTimeout(() => setSnippetCopied(false), 2000)
  }

  const formatJson = () => {
    try {
      setRequestBody(JSON.stringify(JSON.parse(requestBody), null, 2))
    } catch { /* ignore */ }
  }

  const filteredEndpoints = activeCategory === 'all'
    ? endpoints
    : endpoints.filter(e => e.category === activeCategory)

  const actionBtn = 'inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium text-zinc-600 dark:text-zinc-400 border border-zinc-200 dark:border-zinc-800 rounded-md hover:bg-zinc-50 dark:hover:bg-zinc-800 transition-colors'

  return (
    <div className="pt-16">
      <ScrollProgress />

      {/* Hero */}
      <section className="pt-16 pb-10 border-b border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <h1 className="flex items-center gap-3 text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
            <Terminal size={28} className="text-accent-600 dark:text-accent-400" strokeWidth={1.75} />
            API <span className="text-accent-600 dark:text-accent-400">Playground</span>
          </h1>
          <p className="mt-3 max-w-2xl text-zinc-600 dark:text-zinc-400 leading-relaxed">
            Test every UCFP server endpoint in real-time. Every request honors your current
            configuration — update the panel below to change the target server, API key, or
            pipeline parameters.
          </p>
        </div>
      </section>

      {/* Config toggle */}
      <section className="py-6 border-b border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <button
              onClick={() => setShowConfig(!showConfig)}
              className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <Sliders size={14} />
              {showConfig ? 'Hide' : 'Show'} Configuration
              {showConfig ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </button>
            <div className="flex items-center gap-2 text-xs text-zinc-500 dark:text-zinc-500 font-mono">
              <Settings size={12} />
              <code className="text-zinc-700 dark:text-zinc-300">{config.serverUrl}</code>
              <span className="opacity-50">·</span>
              <span>tenant={config.defaultTenant || 'default'}</span>
              <span className="opacity-50">·</span>
              <span>p={config.enablePerceptual ? 'on' : 'off'}</span>
              <span>s={config.enableSemantic ? 'on' : 'off'}</span>
            </div>
          </div>

          <AnimatePresence>
            {showConfig && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                className="overflow-hidden"
              >
                <div className="mt-4">
                  <ConfigPanel />
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </section>

      {/* Main layout */}
      <section className="py-10">
        <div className="mx-auto max-w-6xl px-6 grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-6">
          {/* Sidebar */}
          <aside className="lg:sticky lg:top-20 lg:self-start lg:max-h-[calc(100vh-6rem)] lg:overflow-y-auto no-scrollbar">
            <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-4">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-500 dark:text-zinc-500 mb-3">
                Endpoints
              </h3>

              <div className="flex flex-wrap gap-1 mb-3">
                {categories.map(cat => (
                  <button
                    key={cat.id}
                    onClick={() => setActiveCategory(cat.id)}
                    className={`px-2 py-1 text-[11px] font-medium rounded-md transition-colors ${
                      activeCategory === cat.id
                        ? 'bg-accent-600 text-white'
                        : 'text-zinc-500 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
                    }`}
                  >
                    {cat.label}
                  </button>
                ))}
              </div>

              <div className="space-y-1">
                {filteredEndpoints.map(ep => {
                  const Icon = ep.icon
                  const isActive = selectedEndpoint.id === ep.id
                  return (
                    <button
                      key={ep.id}
                      onClick={() => handleEndpointChange(ep)}
                      className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left transition-colors ${
                        isActive
                          ? 'bg-zinc-100 dark:bg-zinc-800'
                          : 'hover:bg-zinc-50 dark:hover:bg-zinc-800/50'
                      }`}
                    >
                      <span className={`inline-flex items-center justify-center px-1.5 py-0.5 text-[9px] font-semibold rounded ${methodClass(ep.method)}`}>
                        {ep.method}
                      </span>
                      <Icon size={12} className="text-zinc-400 flex-shrink-0" />
                      <span className={`text-xs truncate ${isActive ? 'text-zinc-900 dark:text-zinc-50 font-medium' : 'text-zinc-600 dark:text-zinc-400'}`}>
                        {ep.label}
                      </span>
                    </button>
                  )
                })}
              </div>

              {/* History */}
              <div className="mt-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                <button
                  onClick={() => setShowHistory(!showHistory)}
                  className="w-full flex items-center justify-between text-xs font-medium text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-50 transition-colors"
                >
                  <span className="flex items-center gap-2">
                    <Clock size={12} />
                    History ({history.length})
                  </span>
                  {showHistory ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                </button>

                <AnimatePresence>
                  {showHistory && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: 'auto', opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      className="overflow-hidden mt-2 space-y-1"
                    >
                      {history.length === 0 ? (
                        <p className="text-[11px] text-zinc-400 dark:text-zinc-600 italic">No requests yet</p>
                      ) : (
                        history.map((item, i) => (
                          <div key={i} className="flex items-center gap-2 text-[11px] font-mono">
                            <span className={`font-semibold ${methodClass(item.method).split(' ').filter(c => c.startsWith('text-')).join(' ')}`}>
                              {item.method}
                            </span>
                            <span className="flex-1 truncate text-zinc-600 dark:text-zinc-400">{item.endpoint}</span>
                            <span className={statusClass(item.status)}>{item.status}</span>
                            <span className="text-zinc-400 dark:text-zinc-600">{item.time}ms</span>
                          </div>
                        ))
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </div>
          </aside>

          {/* Main content */}
          <div className="space-y-6">
            {/* Endpoint header */}
            <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-5">
              <div className="flex items-center gap-3 flex-wrap">
                <span className={`inline-flex items-center px-2.5 py-1 text-xs font-semibold rounded ${methodClass(selectedEndpoint.method)}`}>
                  {selectedEndpoint.method}
                </span>
                <code className="flex-1 min-w-0 text-sm font-mono text-zinc-900 dark:text-zinc-50 truncate">
                  {selectedEndpoint.path}
                </code>
                {!selectedEndpoint.requiresAuth && (
                  <span className="inline-flex items-center px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                    public
                  </span>
                )}
              </div>
              <p className="mt-2 text-sm text-zinc-600 dark:text-zinc-400">{selectedEndpoint.description}</p>
            </div>

            {/* Query params */}
            {selectedEndpoint.queryParams && selectedEndpoint.queryParams.length > 0 && (
              <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-5">
                <h4 className="text-xs font-semibold uppercase tracking-wider text-zinc-500 mb-3">Query Parameters</h4>
                <div className="space-y-3">
                  {selectedEndpoint.queryParams.map(qp => (
                    <div key={qp.key}>
                      <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1">{qp.key}</label>
                      <input
                        type={qp.type}
                        placeholder={qp.placeholder}
                        value={queryParams[qp.key] || ''}
                        onChange={e => setQueryParams(prev => ({ ...prev, [qp.key]: e.target.value }))}
                        className="w-full px-3 py-2 text-sm font-mono bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500"
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Request body */}
            {selectedEndpoint.method === 'POST' && (
              <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-5">
                <div className="flex items-center justify-between mb-3">
                  <h4 className="text-xs font-semibold uppercase tracking-wider text-zinc-500">Request Body</h4>
                  <div className="flex items-center gap-2">
                    <button className={actionBtn} onClick={formatJson}>
                      <Terminal size={11} /> Format
                    </button>
                    <button
                      className={actionBtn}
                      onClick={() => setRequestBody(JSON.stringify(selectedEndpoint.defaultBody || {}, null, 2))}
                    >
                      <RefreshCw size={11} /> Reset
                    </button>
                  </div>
                </div>
                <textarea
                  value={requestBody}
                  onChange={e => setRequestBody(e.target.value)}
                  spellCheck={false}
                  rows={12}
                  className="w-full px-3 py-2 text-xs font-mono bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500 resize-y"
                />
              </div>
            )}

            {/* Send button */}
            <button
              onClick={executeRequest}
              disabled={isLoading}
              className="w-full inline-flex items-center justify-center gap-2 px-6 py-3 text-sm font-medium rounded-lg bg-accent-600 hover:bg-accent-700 text-white disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
            >
              {isLoading ? (
                <>
                  <RefreshCw size={14} className="animate-spin" />
                  Sending...
                </>
              ) : (
                <>
                  <Send size={14} />
                  Send Request
                  <ArrowRight size={14} />
                </>
              )}
            </button>

            {/* Code snippet */}
            <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 overflow-hidden">
              <div className="flex items-center justify-between px-5 py-3 border-b border-zinc-200 dark:border-zinc-800">
                <h4 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                  <Code2 size={12} /> Code Snippet
                </h4>
                <div className="flex items-center gap-2">
                  <div className="flex rounded-md bg-zinc-100 dark:bg-zinc-800 p-0.5">
                    {(['curl', 'js', 'python'] as SnippetLang[]).map(lang => (
                      <button
                        key={lang}
                        onClick={() => setSnippetLang(lang)}
                        className={`px-2 py-1 text-[11px] font-medium rounded transition-colors ${
                          snippetLang === lang
                            ? 'bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-50 shadow-sm'
                            : 'text-zinc-500 dark:text-zinc-400'
                        }`}
                      >
                        {lang === 'js' ? 'JavaScript' : lang === 'curl' ? 'curl' : 'Python'}
                      </button>
                    ))}
                  </div>
                  <button className={actionBtn} onClick={handleCopySnippet}>
                    {snippetCopied ? <Check size={11} /> : <Copy size={11} />}
                    {snippetCopied ? 'Copied' : 'Copy'}
                  </button>
                </div>
              </div>
              <pre className="px-5 py-4 text-xs font-mono text-zinc-700 dark:text-zinc-300 overflow-x-auto bg-zinc-50/50 dark:bg-zinc-950/50 whitespace-pre">{snippet}</pre>
            </div>

            {/* Error */}
            <AnimatePresence>
              {error && (
                <motion.div
                  initial={{ opacity: 0, y: -8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  className="flex items-start gap-3 p-4 rounded-xl border border-rose-500/20 bg-rose-500/5"
                >
                  <AlertCircle size={16} className="text-rose-500 flex-shrink-0 mt-0.5" />
                  <div>
                    <strong className="text-sm font-semibold text-rose-600 dark:text-rose-400">Request Failed</strong>
                    <p className="mt-1 text-xs text-rose-600/80 dark:text-rose-400/80">{error}</p>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Response */}
            <AnimatePresence>
              {response && (
                <motion.div
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  ref={responseRef}
                  className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 overflow-hidden"
                >
                  <div className="flex items-center justify-between px-5 py-3 border-b border-zinc-200 dark:border-zinc-800">
                    <div className="flex items-center gap-3">
                      <span className={`text-sm font-mono font-semibold ${statusClass(response.status)}`}>
                        {response.status} {response.status === 200 ? 'OK' : response.status === 401 ? 'Unauthorized' : response.status === 404 ? 'Not Found' : 'Error'}
                      </span>
                      {responseTime !== null && (
                        <span className="inline-flex items-center gap-1 text-xs text-zinc-500 dark:text-zinc-500 font-mono">
                          <Clock size={10} />
                          {responseTime}ms
                        </span>
                      )}
                    </div>
                    <button className={actionBtn} onClick={handleCopyResponse}>
                      {copied ? <Check size={11} /> : <Copy size={11} />}
                      {copied ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                  <pre className="px-5 py-4 text-xs font-mono text-zinc-700 dark:text-zinc-300 overflow-x-auto bg-zinc-50/50 dark:bg-zinc-950/50 max-h-[500px] overflow-y-auto">
                    {typeof response.body === 'string' ? response.body : JSON.stringify(response.body, null, 2)}
                  </pre>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </section>
    </div>
  )
}
