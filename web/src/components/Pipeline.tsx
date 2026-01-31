import { motion } from 'framer-motion'
import { Inbox, TextQuote, Fingerprint, Brain, Database, Search, Check } from 'lucide-react'
import './Pipeline.css'

const stages = [
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
    metric: '~113μs',
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
    metric: '~249μs',
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
    metric: '~143μs',
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
    metric: '~109μs',
    step: '04'
  },
  { 
    icon: Database, 
    name: 'Index', 
    description: 'Storage layer with pluggable backends for persisting fingerprints and embeddings.',
    features: [
      'RocksDB backend',
      'In-memory mode',
      'ANN search (HNSW)',
      'Embedding quantization'
    ],
    metric: '~180μs',
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

export default function Pipeline() {
  return (
    <section id="pipeline" className="pipeline">
      <div className="section-header">
        <h2 className="section-title">
          Six Stage <span className="gradient-text">Pipeline</span>
        </h2>
        <p className="section-subtitle">
          Each stage operates as a standalone crate with clean boundaries.
          Use only what you need, or orchestrate the full workflow.
        </p>
      </div>

      <div className="pipeline-grid">
        {stages.map((stage, index) => (
          <motion.div
            key={stage.name}
            className="pipeline-card"
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, margin: "-50px" }}
            transition={{ duration: 0.4, delay: index * 0.05 }}
          >
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
          </motion.div>
        ))}
      </div>
    </section>
  )
}
