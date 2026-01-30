import { motion } from 'framer-motion'
import { Clock, Gauge, Zap, TrendingUp } from 'lucide-react'
import './Performance.css'

const metrics = [
  { stage: 'Ingest', latency: '~113 us', throughput: 'Validation + Normalization', icon: Clock },
  { stage: 'Canonical', latency: '~249 us', throughput: 'Unicode NFKC + Tokenization', icon: Gauge },
  { stage: 'Perceptual', latency: '~143-708 us', throughput: 'MinHash Fingerprinting', icon: Zap },
  { stage: 'Semantic', latency: '~109 us', throughput: 'Embedding Generation', icon: TrendingUp },
  { stage: 'Index', latency: '~180 us', throughput: 'Storage Operation', icon: Clock },
  { stage: 'Match', latency: '~320 us', throughput: 'Query Execution', icon: Gauge },
]

const endToEnd = [
  { label: 'Single 1,000-word doc', value: '~30ms' },
  { label: 'Large 10,000-word doc', value: '~150ms' },
  { label: 'Batch (100 docs)', value: '~1.7ms/doc' },
  { label: 'Small docs (1,000)', value: '~244us/doc' },
]

export default function Performance() {
  return (
    <section id="performance" className="performance">
      <motion.div
        className="section-header"
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.6 }}
      >
        <h2 className="section-title">
          Blazing <span className="gradient-text">Fast</span> Performance
        </h2>
        <p className="section-subtitle">
          Built in Rust for maximum performance and safety. Benchmarked on a typical
          development machine with unoptimized debug builds.
        </p>
      </motion.div>

      <div className="performance-content">
        <motion.div
          className="metrics-table-container"
          initial={{ opacity: 0, x: -30 }}
          whileInView={{ opacity: 1, x: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
        >
          <h3 className="table-title">Stage Metrics</h3>
          <div className="metrics-table">
            <div className="table-header">
              <span>Stage</span>
              <span>Latency</span>
              <span>Throughput</span>
            </div>
            {metrics.map((metric, index) => (
              <motion.div
                key={metric.stage}
                className="table-row"
                initial={{ opacity: 0, x: -20 }}
                whileInView={{ opacity: 1, x: 0 }}
                viewport={{ once: true }}
                transition={{ duration: 0.4, delay: index * 0.05 }}
              >
                <div className="stage-cell">
                  <metric.icon size={16} />
                  <span>{metric.stage}</span>
                </div>
                <span className="latency-cell">{metric.latency}</span>
                <span className="throughput-cell">{metric.throughput}</span>
              </motion.div>
            ))}
          </div>
        </motion.div>

        <motion.div
          className="end-to-end-container"
          initial={{ opacity: 0, x: 30 }}
          whileInView={{ opacity: 1, x: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6, delay: 0.2 }}
        >
          <h3 className="table-title">End-to-End Performance</h3>
          <div className="end-to-end-cards">
            {endToEnd.map((item, index) => (
              <motion.div
                key={item.label}
                className="end-to-end-card"
                initial={{ opacity: 0, y: 20 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ duration: 0.4, delay: index * 0.1 }}
              >
                <span className="end-to-end-value">{item.value}</span>
                <span className="end-to-end-label">{item.label}</span>
              </motion.div>
            ))}
          </div>

          <div className="performance-note">
            <p>
              Full pipeline processes a 1,000-word document in approximately 30ms.
              Performance scales linearly with document size and batch processing
              achieves even higher throughput.
            </p>
          </div>
        </motion.div>
      </div>
    </section>
  )
}
