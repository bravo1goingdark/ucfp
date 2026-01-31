import { motion } from 'framer-motion'
import { Clock, Gauge, Zap, TrendingUp } from 'lucide-react'
import './Performance.css'

const metrics = [
  { stage: 'Ingest', latency: '~45 μs', throughput: 'Validation + Metadata', icon: Clock },
  { stage: 'Canonical', latency: '~180 μs', throughput: 'Unicode NFKC + Hash', icon: Gauge },
  { stage: 'Perceptual', latency: '~320 μs', throughput: 'MinHash LSH', icon: Zap },
  { stage: 'Semantic', latency: '~8.5 ms', throughput: 'ONNX Embedding', icon: TrendingUp },
  { stage: 'Index', latency: '~95 μs', throughput: 'Upsert Operation', icon: Clock },
  { stage: 'Match', latency: '~450 μs', throughput: 'Similarity Search', icon: Gauge },
]

const endToEnd = [
  { label: 'Small doc (100 words)', value: '~1.2 ms' },
  { label: 'Medium doc (1K words)', value: '~10 ms' },
  { label: 'Large doc (10K words)', value: '~95 ms' },
  { label: 'Batch (100 docs)', value: '~650 μs/doc' },
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
          Built in Rust for maximum performance and safety. Real-world benchmarks on a 
          typical development machine running release builds.
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
              Full pipeline with ONNX semantic embedding processes 1,000 words 
              in ~10ms. Disable semantic stage for ~100x faster processing at 
              ~100μs per document with exact + perceptual matching only.
            </p>
          </div>
        </motion.div>
      </div>
    </section>
  )
}
