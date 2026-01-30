import { motion } from 'framer-motion'
import { Hash, Scan, Brain, Database, Search, Layers } from 'lucide-react'
import './Features.css'

const features = [
  {
    icon: Hash,
    title: 'Exact Hashing',
    description: 'SHA-256 canonical hashes for byte-level identical matching with cryptographic guarantees.',
  },
  {
    icon: Scan,
    title: 'Perceptual Similarity',
    description: 'MinHash signatures and winnowing for detecting near-duplicates and modified content.',
  },
  {
    icon: Brain,
    title: 'Semantic Embeddings',
    description: 'Dense vector embeddings for meaning-based comparison and paraphrase detection.',
  },
  {
    icon: Database,
    title: 'Unified Index',
    description: 'Pluggable storage backends with RocksDB support for scalable content indexing.',
  },
  {
    icon: Search,
    title: 'Smart Matching',
    description: 'Multi-stage matcher combining exact, perceptual, and semantic similarity scores.',
  },
  {
    icon: Layers,
    title: 'Modular Pipeline',
    description: 'Six independent stages that can be used standalone or as a unified workflow.',
  },
]

export default function Features() {
  return (
    <section id="features" className="features">
      <div className="section-header">
        <h2 className="section-title">
          Three Layers of <span className="gradient-text">Intelligence</span>
        </h2>
        <p className="section-subtitle">
          Traditional hashes fail when content changes slightly. UCFP combines three complementary
          approaches for comprehensive content fingerprinting.
        </p>
      </div>

      <div className="features-grid">
        {features.map((feature, index) => (
          <motion.div
            key={feature.title}
            className="feature-card"
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, margin: "-50px" }}
            transition={{ duration: 0.4, delay: index * 0.05 }}
          >
            <div className="feature-icon-wrapper">
              <feature.icon size={22} strokeWidth={1.5} />
            </div>
            <h3>{feature.title}</h3>
            <p>{feature.description}</p>
          </motion.div>
        ))}
      </div>
    </section>
  )
}
