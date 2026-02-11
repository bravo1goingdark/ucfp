import { useEffect, useState } from 'react'
import { motion, useScroll, useTransform } from 'framer-motion'
import { Github, GitCommit, Clock, Check, AlertCircle, FileText, Image, Music, Video, FileStack, Box, 
         Inbox, TextQuote, Fingerprint, Brain, Database, Search, Zap, Layers, Sparkles } from 'lucide-react'
import '../styles/LandingPage.css'
import '../styles/Pipeline.css'
import { ParticlesBackground, FloatingShapes, NoiseOverlay } from '../components/VisualEffects'
import { useScrollReveal } from '../hooks/useAnimations'

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
    icon: Inbox, 
    name: 'Ingest', 
    description: 'The entry point handles raw content with validation, normalization, and metadata extraction.',
    features: [
      'Multi-format input support',
      'Metadata extraction',
      'Schema validation',
      'ID derivation'
    ],
    metric: '~49μs',
    step: '01'
  },
  { 
    icon: TextQuote, 
    name: 'Canonical', 
    description: 'Transforms content into standardized form with Unicode normalization and tokenization.',
    features: [
      'Unicode NFKC normalization',
      'Smart tokenization',
      'SHA-256 hashing',
      'Deduplication ready'
    ],
    metric: '~195μs',
    step: '02'
  },
  { 
    icon: Fingerprint, 
    name: 'Perceptual', 
    description: 'Generates fingerprints for detecting similar and modified content beyond exact matches.',
    features: [
      'Rolling hash shingles',
      'Winnowing algorithm',
      'MinHash LSH bands',
      'Parallel processing'
    ],
    metric: '~82μs',
    step: '03'
  },
  { 
    icon: Brain, 
    name: 'Semantic', 
    description: 'Creates dense vector embeddings capturing semantic meaning for meaning-based search.',
    features: [
      'ONNX runtime support',
      'BGE/E5 embeddings',
      'API fallback mode',
      'Sentence transformers'
    ],
    metric: '~1.1ms',
    step: '04'
  },
  { 
    icon: Database, 
    name: 'Index', 
    description: 'Storage layer with pluggable backends for persisting fingerprints and embeddings.',
    features: [
      'Redb backend',
      'In-memory mode',
      'ANN search (HNSW)',
      'Embedding quantization'
    ],
    metric: '~50μs',
    step: '05'
  },
  { 
    icon: Search, 
    name: 'Match', 
    description: 'Query-time engine combining multiple matching strategies for accurate results.',
    features: [
      'Multi-strategy search',
      'Similarity scoring',
      'Threshold tuning',
      'Real-time results'
    ],
    metric: '~320μs',
    step: '06'
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

const faqs = [
  {
    question: 'Is this production-ready?',
    answer: 'The text pipeline is ready for production. We are running it internally and with a few early users. Image, audio, and video support are still in development. If you need those modalities today, UCFP is not the right fit yet.'
  },
  {
    question: 'How does pricing work?',
    answer: 'UCFP is open source under Apache 2.0. Free to use, modify, and distribute. If you use the semantic embedding stage with external APIs (OpenAI, etc.), you pay their rates. ONNX local embeddings are free. We are also planning to launch a managed SaaS version for teams that prefer a hosted solution without infrastructure overhead.'
  },
  {
    question: 'Will there be a hosted/SaaS version?',
    answer: 'Yes! We are actively working on launching a managed SaaS platform that will provide UCFP as a service. This will include hosted API endpoints, automatic scaling, managed infrastructure, and a web dashboard for managing fingerprints and viewing analytics. Early access signup is coming soon.'
  },
  {
    question: 'What languages are supported?',
    answer: 'The core library is Rust. The REST API server can be called from any language. We have examples in Python, JavaScript, and Go in the repository.'
  },
  {
    question: 'Can I disable stages I do not need?',
    answer: 'Yes. Each stage is optional at runtime. Disable semantic embeddings for ~100x faster processing if you only need exact and perceptual matching. Disable perceptual for cryptographic-only deduplication.'
  },
  {
    question: 'How do I deploy this?',
    answer: 'Two options: (1) Embed the Rust library directly in your application, or (2) run the standalone HTTP server and call it via REST. The server is a single binary with no dependencies. For teams that prefer a managed solution, our upcoming SaaS platform will provide hosted API endpoints with automatic scaling and no infrastructure management required.'
  },
  {
    question: 'What is the throughput at scale?',
    answer: 'With all stages enabled: ~125 docs/second for 1K-word documents on a typical 4-core machine. Disabling semantic stage: ~10,000 docs/second. Batch processing and lock-free indexing help maintain throughput under concurrent load.'
  }
]



function AnimatedCounter({ value, suffix = '' }: { value: number; suffix?: string }) {
  const [count, setCount] = useState(0)
  const { ref, isRevealed } = useScrollReveal<HTMLSpanElement>({ threshold: 0.5 })

  useEffect(() => {
    if (!isRevealed) return

    const duration = 2000
    const startTime = performance.now()

    const animate = (currentTime: number) => {
      const elapsed = currentTime - startTime
      const progress = Math.min(elapsed / duration, 1)
      const easeOut = 1 - Math.pow(1 - progress, 3)
      setCount(Math.floor(value * easeOut))

      if (progress < 1) {
        requestAnimationFrame(animate)
      }
    }

    requestAnimationFrame(animate)
  }, [isRevealed, value])

  return (
    <span ref={ref}>
      {count}
      {suffix}
    </span>
  )
}

export default function LandingPage() {
  const [commit, setCommit] = useState<CommitInfo | null>(null)
  const { scrollYProgress } = useScroll()
  const heroY = useTransform(scrollYProgress, [0, 0.3], [0, -100])
  const heroOpacity = useTransform(scrollYProgress, [0, 0.3], [1, 0])

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
            timeAgo: getTimeAgo(commitDate)
          })
        }
      })
      .catch(() => null)
  }, [])

  return (
    <div className="landing-page" style={{ position: 'relative' }}>
      <ParticlesBackground />
      <FloatingShapes />
      <NoiseOverlay />
      {/* 1. HERO SECTION */}
      <motion.section 
        className="hero-section"
        style={{ y: heroY, opacity: heroOpacity }}
      >
        <motion.div 
          className="hero-badge-container"
          initial={{ opacity: 0, y: 30, scale: 0.9 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          transition={{ duration: 0.8, ease: [0.16, 1, 0.3, 1] }}
        >
          <div className="badge-group glow-sm">
            <span className="hero-badge open-source-badge">
              <span className="pulse-dot" />
              Open Source
            </span>
            
            {commit && (
              <motion.a
                href={`https://github.com/bravo1goingdark/ucfp/commit/${commit.sha}`}
                target="_blank"
                rel="noopener noreferrer"
                className="hero-badge commit-badge"
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ duration: 0.6, delay: 0.3, ease: [0.16, 1, 0.3, 1] }}
                whileHover={{ scale: 1.02 }}
              >
                <GitCommit size={14} />
                <code className="commit-hash">{commit.sha}</code>
                <span className="commit-divider" />
                <span className="commit-message">{commit.message}</span>
                <span className="commit-time">
                  <Clock size={12} />
                  {commit.timeAgo}
                </span>
              </motion.a>
            )}
          </div>
        </motion.div>

        <motion.h1
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.1, ease: [0.16, 1, 0.3, 1] }}
        >
          Universal Content
          <br />
          <span className="gradient-text">
            Fingerprinting
            <Sparkles className="float" size={32} style={{ 
              display: 'inline-block', 
              marginLeft: '12px',
              color: 'var(--accent-amber)',
              verticalAlign: 'middle'
            }} />
          </span>
        </motion.h1>

        <motion.p
          className="hero-subtitle"
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.2, ease: [0.16, 1, 0.3, 1] }}
        >
          Deterministic fingerprints for text, images, audio, video, and documents.
          Built in Rust for teams that need exact matching, near-duplicate detection,
          and semantic search in one pipeline. Self-hosted or managed SaaS coming soon.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.3, ease: [0.16, 1, 0.3, 1] }}
        >
          <motion.a 
            href="https://github.com/bravo1goingdark/ucfp" 
            className="btn btn-gradient btn-large"
            whileHover={{ scale: 1.05, y: -3 }}
            whileTap={{ scale: 0.98 }}
          >
            <Github size={18} />
            Get Started on GitHub
          </motion.a>
        </motion.div>

        <motion.p
          className="hero-note"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.8, delay: 0.5 }}
        >
          ~2ms per 1K words · 6-stage pipeline · REST API included
        </motion.p>
      </motion.section>

      {/* 2. HOW IT WORKS - 6 Pipeline Stages */}
      <section id="pipeline" className="content-section how-it-works-section">
        <div>
          <div className="section-header">
            <h2>Six Stage <span className="gradient-text">Pipeline</span></h2>
            <p className="section-subtitle">
              Each stage operates as a standalone crate with clean boundaries.
              Use only what you need, or orchestrate the full workflow.
            </p>
          </div>
          
          <div className="pipeline-grid">
            {pipelineStages.map((stage) => (
              <div key={stage.name} className="pipeline-card-wrapper">
                <div className="pipeline-card card-gradient-border">
                  <div className="pipeline-card-header">
                    <div className="pipeline-icon">
                      <stage.icon size={22} strokeWidth={1.5} />
                    </div>
                    <span className="pipeline-step">{stage.step}</span>
                  </div>
                  
                  <div className="pipeline-card-body">
                    <h3>{stage.name}</h3>
                    <p className="pipeline-description">{stage.description}</p>
                    
                    <ul className="pipeline-features">
                      {stage.features.map((feature, fIndex) => (
                        <li key={fIndex}>
                          <Check size={14} />
                          {feature}
                        </li>
                      ))}
                    </ul>
                  </div>

                  <div className="pipeline-card-footer">
                    <span className="pipeline-metric">{stage.metric}</span>
                    <span className="pipeline-metric-label">avg latency</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* 5. KEY BENEFITS */}
      <section id="benefits" className="content-section benefits-section">
        <div>
          <h2>Key Benefits</h2>
          
          <ul className="benefits-list">
            <li>
              <strong>Three matching strategies in one call.</strong>
              Exact cryptographic hashes, perceptual fingerprints for near-duplicates,
              and semantic embeddings for paraphrase detection. No need to run separate systems.
            </li>
            
            <li>
              <strong>Predictable performance.</strong>
              ~8ms end-to-end for 1,000 words on commodity hardware.
              Lock-free indexing and parallel processing keep latency consistent under load.
            </li>
            
            <li>
              <strong>Deterministic and reproducible.</strong>
              Same input produces identical fingerprints across environments.
              Essential for content provenance and audit trails.
            </li>
            
            <li>
              <strong>Pluggable architecture.</strong>
              Each stage is a standalone crate. Use only what you need.
              Swap storage backends (in-memory, Redb) without code changes.
            </li>
            
            <li>
              <strong>Resilient by design.</strong>
              Circuit breakers, exponential backoff, and rate limiting for external embedding APIs.
              Your pipeline stays up even when upstream services degrade.
            </li>
            
            <li>
              <strong>Available as library or server.</strong>
              Embed directly in your Rust application or deploy the HTTP server
              for teams using other languages.
            </li>
          </ul>
        </div>
      </section>

      {/* 6. LIMITATIONS / NON-GOALS */}
      <section className="content-section limitations-section">
        <div>
          <h2>What UCFP Does Not Do</h2>
          
          <ul className="limitations-list">
            <li>
              <AlertCircle size={20} />
              <div>
                <strong>Only text is production-ready.</strong>
                Image, audio, video, document, and 3D model support are on the roadmap
                but not yet implemented.
              </div>
            </li>
            
            <li>
              <AlertCircle size={20} />
              <div>
                <strong>Not a general-purpose vector database.</strong>
                UCFP indexes content fingerprints, not arbitrary vectors.
                Use Pinecone, Weaviate, or pgvector if you need generic vector search.
              </div>
            </li>
            
            <li>
              <AlertCircle size={20} />
              <div>
                <strong>No built-in distributed coordination.</strong>
                Single-node deployment only. Clustering and replication are not supported yet.
              </div>
            </li>
            
            <li>
              <AlertCircle size={20} />
              <div>
                <strong>Not a content management system.</strong>
                UCFP generates and matches fingerprints. It does not store original content,
                handle user permissions, or manage content workflows.
              </div>
            </li>
          </ul>
        </div>
      </section>

      {/* 7. SOCIAL PROOF / STATUS - Bigger Cards */}
      <section id="status" className="content-section status-section">
        <div>
          <h2>Current Status</h2>
          
          <p className="status-text">
            Early access. The text pipeline is stable and used in production by a small number
            of teams for deduplication and content search. A managed SaaS platform is in development
            for teams that prefer hosted solutions. Image, audio, and video modalities are planned for 2025.
          </p>
          
          <div className="status-stats-large">
            <div className="stat-card-wrapper">
              <div className="stat-card-large glow-sm">
                <div className="stat-icon pulse-glow">
                  <Zap size={32} />
                </div>
                <span className="stat-value">~2ms</span>
                <span className="stat-label">per 1K words</span>
                <span className="stat-sublabel">End-to-end processing</span>
              </div>
            </div>
            
            <div className="stat-card-wrapper">
              <div className="stat-card-large glow-sm">
                <div className="stat-icon">
                  <Layers size={32} />
                </div>
                <span className="stat-value">6</span>
                <span className="stat-label">pipeline stages</span>
                <span className="stat-sublabel">Independent & modular</span>
              </div>
            </div>
            
            <div className="stat-card-wrapper">
              <div className="stat-card-large glow-sm">
                <div className="stat-icon ready pulse-glow">
                  <Check size={32} />
                </div>
                <span className="stat-value">1</span>
                <span className="stat-label">ready modality</span>
                <span className="stat-sublabel">Text pipeline stable</span>
              </div>
            </div>
          </div>

          <div className="modalities-grid-large">
            {modalities.map((modality) => (
              <div key={modality.name} className="modality-card-wrapper">
                <div className={`modality-card-large ${modality.status === 'Ready' ? 'card-gradient-border' : ''}`}>
                  <div className="modality-header-large">
                    <div className="modality-icon-large">
                      <modality.icon size={28} strokeWidth={1.5} />
                    </div>
                    <span className={`modality-status ${modality.status.toLowerCase()}`}>
                      {modality.status === 'Ready' ? <Check size={14} /> : <Clock size={14} />}
                      {modality.status}
                    </span>
                  </div>
                  <h4>{modality.name}</h4>
                  <p>{modality.description}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* 8. FAQ */}
      <section id="faq" className="content-section faq-section">
        <div>
          <h2>FAQ</h2>
          
          <div className="faq-list">
            {faqs.map((faq, index) => (
              <div key={index} className="faq-item">
                <h3>{faq.question}</h3>
                <p>{faq.answer}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* 9. FINAL CTA */}
      <section className="final-cta-section">
        <div>
          <h2>
            Built for teams that need
            <br />
            <span className="gradient-text">content matching</span> they can trust.
          </h2>
          
          <p>
            If you are building deduplication, plagiarism detection, or semantic search
            into your product and want one system that handles exact, similar, and semantic
            matching, UCFP is designed for you.
          </p>
          
          <div>
            <a 
              href="https://github.com/bravo1goingdark/ucfp" 
              className="btn btn-gradient btn-large pulse-glow"
            >
              <Github size={18} />
              Get Started on GitHub
            </a>
          </div>
          
          <p className="cta-note">
            Apache 2.0 licensed · Text pipeline ready · SaaS launch coming soon · Other modalities in development
          </p>
        </div>
      </section>
    </div>
  )
}
