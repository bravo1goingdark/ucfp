import { motion } from 'framer-motion'
import { Clock, Gauge, Zap, TrendingUp, Network, Shield } from 'lucide-react'
import './Performance.css'

const metrics = [
  { stage: 'Ingest', latency: '~45 μs', throughput: 'Validation + Metadata', icon: Clock },
  { stage: 'Canonical', latency: '~180 μs', throughput: 'Unicode NFKC + Hash', icon: Gauge },
  { stage: 'Perceptual', latency: '~180 μs', throughput: 'Parallel MinHash LSH', icon: Zap, highlight: true },
  { stage: 'Semantic', latency: '~8.5 ms', throughput: 'Async ONNX Embedding', icon: TrendingUp, highlight: true },
  { stage: 'Index', latency: '~50 μs', throughput: 'Lock-free Upsert', icon: Network, highlight: true },
  { stage: 'Match', latency: '~50 μs*', throughput: 'ANN O(log n) Search', icon: Shield, highlight: true },
]

const endToEnd = [
  { label: 'Small doc (100 words)', value: '~1.0 ms', improvement: '15% faster' },
  { label: 'Medium doc (1K words)', value: '~8 ms', improvement: '20% faster' },
  { label: 'Large doc (10K words)', value: '~75 ms', improvement: '20% faster' },
  { label: 'Batch (100 docs)', value: '~400 μs/doc', improvement: '1.6x faster' },
  { label: 'ANN Search (10K docs)', value: '~100 μs', improvement: '50x faster' },
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
          High-throughput optimizations deliver 5-10x performance gains. Built in Rust 
          with lock-free concurrency, ANN search, and SIMD vectorization.
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
                className={`table-row ${metric.highlight ? 'highlight-row' : ''}`}
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
          <p className="table-footnote">* ANN search latency for datasets with 1000+ vectors</p>
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
                <span className="end-to-end-improvement">{item.improvement}</span>
              </motion.div>
            ))}
          </div>

          <div className="performance-highlights">
            <div className="highlight-item">
              <Zap size={20} />
              <span><strong>5-10x</strong> index throughput with lock-free DashMap</span>
            </div>
            <div className="highlight-item">
              <Network size={20} />
              <span><strong>100-1000x</strong> semantic search with HNSW ANN</span>
            </div>
            <div className="highlight-item">
              <TrendingUp size={20} />
              <span><strong>10x</strong> batch processing with parallel execution</span>
            </div>
          </div>

          <div className="performance-note">
            <p>
              Full pipeline with ONNX semantic embedding processes 1,000 words 
              in ~8ms. ANN search automatically activates for indexes with 1000+ vectors,
              delivering sub-linear O(log n) performance. Disable semantic stage for 
              ~100x faster processing at ~100μs per document.
            </p>
          </div>
        </motion.div>
      </div>
    </section>
  )
}
