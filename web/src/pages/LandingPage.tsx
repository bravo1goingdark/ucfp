import { useEffect, useState } from 'react'
import { motion } from 'framer-motion'
import { Link } from 'react-router-dom'
import {
  Github, GitCommit, Clock, Check, AlertCircle, FileText, Image, Music, Video, FileStack, Box,
  Inbox, TextQuote, Fingerprint, Brain, Database, Search, Zap, Layers,
  Terminal, LayoutDashboard, ArrowRight,
} from 'lucide-react'
import InteractivePipelineDemo from '../components/InteractivePipelineDemo'

interface CommitInfo {
  sha: string
  message: string
  date: string
  timeAgo: string
}

function getTimeAgo(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const seconds = Math.floor((now.getTime() - date.getTime()) / 1000)
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`
  return date.toLocaleDateString()
}

const pipelineStages = [
  {
    icon: Inbox, name: 'Ingest', step: '01', metric: '~49μs',
    description: 'Entry point — validates raw content, extracts metadata, derives IDs.',
    features: ['Multi-format input', 'Metadata extraction', 'Schema validation', 'ID derivation'],
  },
  {
    icon: TextQuote, name: 'Canonical', step: '02', metric: '~195μs',
    description: 'Transforms content to standardized form with Unicode normalization.',
    features: ['NFKC normalization', 'Smart tokenization', 'SHA-256 hashing', 'Deduplication ready'],
  },
  {
    icon: Fingerprint, name: 'Perceptual', step: '03', metric: '~82μs',
    description: 'Fingerprints for detecting similar and modified content beyond exact matches.',
    features: ['Rolling hash shingles', 'Winnowing', 'MinHash LSH bands', 'Parallel processing'],
  },
  {
    icon: Brain, name: 'Semantic', step: '04', metric: '~1.1ms',
    description: 'Dense vector embeddings capturing semantic meaning for meaning-based search.',
    features: ['ONNX runtime', 'BGE/E5 embeddings', 'API fallback mode', 'Sentence transformers'],
  },
  {
    icon: Database, name: 'Index', step: '05', metric: '~50μs',
    description: 'Pluggable storage for persisting fingerprints and embeddings.',
    features: ['Redb backend', 'In-memory mode', 'ANN search (HNSW)', 'Quantization'],
  },
  {
    icon: Search, name: 'Match', step: '06', metric: '~320μs',
    description: 'Query-time engine combining multiple matching strategies.',
    features: ['Multi-strategy', 'Similarity scoring', 'Threshold tuning', 'Real-time results'],
  },
]

const modalities = [
  { icon: FileText, name: 'Text', status: 'Ready', description: 'Full text support with Unicode normalization and semantic embeddings' },
  { icon: Image, name: 'Image', status: 'Planned', description: 'Perceptual hashing and vision-language embeddings' },
  { icon: Music, name: 'Audio', status: 'Planned', description: 'Audio fingerprinting with mel-frequency cepstral coefficients' },
  { icon: Video, name: 'Video', status: 'Planned', description: 'Video scene detection and temporal fingerprinting' },
  { icon: FileStack, name: 'Document', status: 'Planned', description: 'PDF and document layout understanding with OCR' },
  { icon: Box, name: '3D Model', status: 'Planned', description: '3D model fingerprinting for mesh comparison' },
]

const benefits = [
  {
    title: 'Three matching strategies in one call',
    body: 'Exact cryptographic hashes, perceptual fingerprints for near-duplicates, and semantic embeddings for paraphrase detection. No need to run separate systems.',
  },
  {
    title: 'Predictable performance',
    body: '~8ms end-to-end for 1,000 words on commodity hardware. Lock-free indexing and parallel processing keep latency consistent under load.',
  },
  {
    title: 'Deterministic and reproducible',
    body: 'Same input produces identical fingerprints across environments. Essential for content provenance and audit trails.',
  },
  {
    title: 'Pluggable architecture',
    body: 'Each stage is a standalone crate. Use only what you need. Swap storage backends without code changes.',
  },
  {
    title: 'Resilient by design',
    body: 'Circuit breakers, exponential backoff, and rate limiting for external embedding APIs. Your pipeline stays up when upstream degrades.',
  },
  {
    title: 'Library or server',
    body: 'Embed directly in your Rust application or deploy the HTTP server for teams using other languages.',
  },
]

const limitations = [
  {
    title: 'Only text is production-ready',
    body: 'Image, audio, video, document, and 3D model support are on the roadmap but not yet implemented.',
  },
  {
    title: 'Not a general-purpose vector database',
    body: 'UCFP indexes content fingerprints, not arbitrary vectors. Use Pinecone, Weaviate, or pgvector for generic vector search.',
  },
  {
    title: 'No built-in distributed coordination',
    body: 'Single-node deployment only. Clustering and replication are not supported yet.',
  },
  {
    title: 'Not a content management system',
    body: 'UCFP generates and matches fingerprints. It does not store original content or manage workflows.',
  },
]

const faqs = [
  {
    question: 'Is this production-ready?',
    answer: 'The text pipeline is ready for production. We are running it internally and with a few early users. Image, audio, and video support are still in development.',
  },
  {
    question: 'How does pricing work?',
    answer: 'UCFP is open source under Apache 2.0. Free to use, modify, and distribute. External embedding APIs are paid at provider rates; ONNX local embeddings are free.',
  },
  {
    question: 'Will there be a hosted/SaaS version?',
    answer: 'Yes. We are actively working on launching a managed SaaS platform with hosted API endpoints, automatic scaling, and a web dashboard.',
  },
  {
    question: 'What languages are supported?',
    answer: 'The core library is Rust. The REST API server can be called from any language. Examples in Python, JavaScript, and Go live in the repository.',
  },
  {
    question: 'Can I disable stages I do not need?',
    answer: 'Yes. Each stage is optional at runtime. Disable semantic for ~100x faster processing if you only need exact and perceptual matching.',
  },
  {
    question: 'How do I deploy this?',
    answer: 'Embed the Rust library directly, or run the standalone HTTP server and call it via REST. The server is a single binary with no dependencies.',
  },
  {
    question: 'What is the throughput at scale?',
    answer: 'With all stages enabled: ~125 docs/s for 1K-word documents on a 4-core machine. Semantic disabled: ~10,000 docs/s.',
  },
]

export default function LandingPage() {
  const [commit, setCommit] = useState<CommitInfo | null>(null)

  useEffect(() => {
    fetch('https://api.github.com/repos/bravo1goingdark/ucfp/commits?per_page=1')
      .then(res => res.json())
      .then(data => {
        if (data && data[0]) {
          const commitDate = data[0].commit.committer.date
          setCommit({
            sha: data[0].sha.substring(0, 7),
            message: data[0].commit.message.split('\n')[0],
            date: commitDate,
            timeAgo: getTimeAgo(commitDate),
          })
        }
      })
      .catch(() => null)
  }, [])

  return (
    <div className="relative">
      {/* HERO */}
      <section className="relative pt-32 pb-20 sm:pt-40 sm:pb-28">
        <div className="mx-auto max-w-4xl px-6 text-center">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            className="flex flex-wrap items-center justify-center gap-2 mb-8"
          >
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border border-emerald-500/20 bg-emerald-500/5 text-xs font-medium text-emerald-600 dark:text-emerald-400">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
              Open Source
            </span>
            {commit && (
              <a
                href={`https://github.com/bravo1goingdark/ucfp/commit/${commit.sha}`}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-xs text-zinc-600 dark:text-zinc-400 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
              >
                <GitCommit size={12} />
                <code className="font-mono text-zinc-700 dark:text-zinc-300">{commit.sha}</code>
                <span className="hidden sm:inline opacity-50">·</span>
                <span className="hidden sm:inline max-w-[200px] truncate">{commit.message}</span>
                <span className="opacity-50">·</span>
                <Clock size={10} />
                <span>{commit.timeAgo}</span>
              </a>
            )}
          </motion.div>

          <motion.h1
            initial={{ opacity: 0, y: 24 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.1, ease: [0.16, 1, 0.3, 1] }}
            className="text-4xl sm:text-5xl md:text-6xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50 leading-[1.05]"
          >
            Universal Content
            <br />
            <span className="text-accent-600 dark:text-accent-400">Fingerprinting</span>
          </motion.h1>

          <motion.p
            initial={{ opacity: 0, y: 24 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.2, ease: [0.16, 1, 0.3, 1] }}
            className="mt-6 max-w-2xl mx-auto text-base sm:text-lg leading-relaxed text-zinc-600 dark:text-zinc-400"
          >
            Deterministic fingerprints for text, images, audio, video, and documents.
            Built in Rust for teams that need exact matching, near-duplicate detection,
            and semantic search in one pipeline.
          </motion.p>

          <motion.div
            initial={{ opacity: 0, y: 24 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.3, ease: [0.16, 1, 0.3, 1] }}
            className="mt-10 flex flex-wrap items-center justify-center gap-3"
          >
            <Link
              to="/playground"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg bg-zinc-900 dark:bg-zinc-50 text-white dark:text-zinc-900 text-sm font-medium hover:bg-zinc-800 dark:hover:bg-zinc-200 transition-colors"
            >
              <Terminal size={16} />
              Try the Playground
              <ArrowRight size={14} />
            </Link>
            <Link
              to="/dashboard"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 text-sm font-medium hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <LayoutDashboard size={16} />
              Dashboard
            </Link>
            <a
              href="https://github.com/bravo1goingdark/ucfp"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 text-sm font-medium hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <Github size={16} />
              GitHub
            </a>
          </motion.div>

          <motion.p
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.7, delay: 0.5 }}
            className="mt-8 text-xs text-zinc-400 dark:text-zinc-600 font-mono"
          >
            ~2ms per 1K words · 6-stage pipeline · REST API included
          </motion.p>
        </div>
      </section>

      {/* PIPELINE */}
      <section id="pipeline" className="py-16 sm:py-24 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <div className="max-w-2xl mb-10">
            <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
              Six-stage pipeline
            </h2>
            <p className="mt-3 text-zinc-600 dark:text-zinc-400 leading-relaxed">
              Each stage operates as a standalone crate with clean boundaries.
              Use only what you need, or orchestrate the full workflow.
            </p>
          </div>

          <InteractivePipelineDemo />

          <div className="mt-12 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {pipelineStages.map((stage, i) => (
              <motion.div
                key={stage.name}
                initial={{ opacity: 0, y: 16 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: '-40px' }}
                transition={{ duration: 0.4, delay: i * 0.04 }}
                className="group relative rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-5 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-accent-500/10 text-accent-600 dark:text-accent-400">
                    <stage.icon size={18} strokeWidth={1.75} />
                  </div>
                  <span className="text-xs font-mono text-zinc-400 dark:text-zinc-600">{stage.step}</span>
                </div>
                <h3 className="text-base font-semibold text-zinc-900 dark:text-zinc-50">{stage.name}</h3>
                <p className="mt-1.5 text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">{stage.description}</p>
                <ul className="mt-4 space-y-1.5">
                  {stage.features.map((feature) => (
                    <li key={feature} className="flex items-center gap-2 text-xs text-zinc-500 dark:text-zinc-500">
                      <Check size={12} className="text-emerald-500 flex-shrink-0" />
                      {feature}
                    </li>
                  ))}
                </ul>
                <div className="mt-5 pt-4 border-t border-zinc-100 dark:border-zinc-800 flex items-center justify-between">
                  <span className="text-sm font-mono font-semibold text-zinc-900 dark:text-zinc-50">{stage.metric}</span>
                  <span className="text-[10px] uppercase tracking-wider text-zinc-400 dark:text-zinc-600">avg latency</span>
                </div>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* BENEFITS */}
      <section id="benefits" className="py-16 sm:py-24 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <div className="max-w-2xl mb-10">
            <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
              Key benefits
            </h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-10 gap-y-8">
            {benefits.map((benefit, i) => (
              <motion.div
                key={benefit.title}
                initial={{ opacity: 0, y: 16 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: '-40px' }}
                transition={{ duration: 0.4, delay: i * 0.04 }}
                className="flex gap-4"
              >
                <div className="flex-shrink-0 mt-0.5 flex items-center justify-center w-7 h-7 rounded-md bg-accent-500/10 text-accent-600 dark:text-accent-400">
                  <Check size={14} strokeWidth={2.5} />
                </div>
                <div>
                  <h3 className="text-base font-semibold text-zinc-900 dark:text-zinc-50">{benefit.title}</h3>
                  <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">{benefit.body}</p>
                </div>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* LIMITATIONS */}
      <section className="py-16 sm:py-24 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <div className="max-w-2xl mb-10">
            <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
              What UCFP does not do
            </h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-10 gap-y-8">
            {limitations.map((limit) => (
              <div key={limit.title} className="flex gap-4">
                <div className="flex-shrink-0 mt-0.5 flex items-center justify-center w-7 h-7 rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400">
                  <AlertCircle size={14} />
                </div>
                <div>
                  <h3 className="text-base font-semibold text-zinc-900 dark:text-zinc-50">{limit.title}</h3>
                  <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">{limit.body}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* STATUS */}
      <section id="status" className="py-16 sm:py-24 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-6xl px-6">
          <div className="max-w-2xl mb-10">
            <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
              Current status
            </h2>
            <p className="mt-3 text-zinc-600 dark:text-zinc-400 leading-relaxed">
              Early access. The text pipeline is stable and used in production by a small number
              of teams. Image, audio, and video modalities are planned for 2025.
            </p>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-10">
            {[
              { icon: Zap, value: '~2ms', label: 'per 1K words', sub: 'End-to-end processing' },
              { icon: Layers, value: '6', label: 'pipeline stages', sub: 'Independent & modular' },
              { icon: Check, value: '1', label: 'ready modality', sub: 'Text pipeline stable' },
            ].map((stat) => (
              <div
                key={stat.label}
                className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-6"
              >
                <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-accent-500/10 text-accent-600 dark:text-accent-400">
                  <stat.icon size={18} strokeWidth={1.75} />
                </div>
                <div className="mt-4 text-3xl font-semibold text-zinc-900 dark:text-zinc-50">{stat.value}</div>
                <div className="mt-1 text-sm font-medium text-zinc-700 dark:text-zinc-300">{stat.label}</div>
                <div className="text-xs text-zinc-500 dark:text-zinc-500">{stat.sub}</div>
              </div>
            ))}
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {modalities.map((modality) => (
              <div
                key={modality.name}
                className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 p-5"
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400">
                    <modality.icon size={18} strokeWidth={1.75} />
                  </div>
                  <span
                    className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium uppercase tracking-wider ${
                      modality.status === 'Ready'
                        ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                        : 'bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-500'
                    }`}
                  >
                    {modality.status === 'Ready' ? <Check size={10} /> : <Clock size={10} />}
                    {modality.status}
                  </span>
                </div>
                <h4 className="text-base font-semibold text-zinc-900 dark:text-zinc-50">{modality.name}</h4>
                <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-500 leading-relaxed">{modality.description}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* FAQ */}
      <section id="faq" className="py-16 sm:py-24 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-3xl px-6">
          <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50 mb-10">
            FAQ
          </h2>
          <div className="space-y-2">
            {faqs.map((faq, i) => (
              <details
                key={i}
                className="group rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900/50 px-5 py-4 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
              >
                <summary className="flex items-center justify-between cursor-pointer list-none">
                  <span className="text-base font-medium text-zinc-900 dark:text-zinc-50">{faq.question}</span>
                  <span className="ml-4 text-zinc-400 group-open:rotate-180 transition-transform">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                      <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  </span>
                </summary>
                <p className="mt-3 text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">{faq.answer}</p>
              </details>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="py-20 sm:py-28 border-t border-zinc-200 dark:border-zinc-800/80">
        <div className="mx-auto max-w-3xl px-6 text-center">
          <h2 className="text-3xl sm:text-4xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50 leading-tight">
            Built for teams that need
            <br />
            <span className="text-accent-600 dark:text-accent-400">content matching</span> they can trust.
          </h2>
          <p className="mt-5 text-zinc-600 dark:text-zinc-400 leading-relaxed">
            If you are building deduplication, plagiarism detection, or semantic search
            into your product, UCFP is designed for you.
          </p>
          <div className="mt-10 flex flex-wrap items-center justify-center gap-3">
            <Link
              to="/playground"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg bg-zinc-900 dark:bg-zinc-50 text-white dark:text-zinc-900 text-sm font-medium hover:bg-zinc-800 dark:hover:bg-zinc-200 transition-colors"
            >
              <Terminal size={16} />
              Try the Playground
              <ArrowRight size={14} />
            </Link>
            <Link
              to="/dashboard"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 text-sm font-medium hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <LayoutDashboard size={16} />
              Dashboard
            </Link>
            <a
              href="https://github.com/bravo1goingdark/ucfp"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 text-sm font-medium hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors"
            >
              <Github size={16} />
              GitHub
            </a>
          </div>
          <p className="mt-6 text-xs text-zinc-400 dark:text-zinc-600">
            Apache 2.0 · Text pipeline ready · Other modalities in development
          </p>
        </div>
      </section>
    </div>
  )
}
