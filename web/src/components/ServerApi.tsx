import { motion } from 'framer-motion'
import { Globe, Shield, Zap } from 'lucide-react'
import './ServerApi.css'

const features = [
  { icon: Globe, title: 'REST API', desc: 'HTTP endpoints for all pipeline operations' },
  { icon: Shield, title: 'Authentication', desc: 'API key-based auth with tenant isolation' },
  { icon: Zap, title: 'Async I/O', desc: 'Built on Tokio for high-performance requests' },
]

export default function ServerApi() {
  return (
    <section id="server" className="server-api">
      <div className="section-header">
        <h2 className="section-title">
          REST <span className="gradient-text">API Server</span>
        </h2>
        <p className="section-subtitle">
          Deploy UCFP as a standalone HTTP server. Perfect for microservices and language-agnostic integrations.
        </p>
      </div>

      <div className="server-content">
        <motion.div
          className="server-card"
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <div className="server-card-header">
            <h3>HTTP Endpoints</h3>
            <span className="server-badge">7 endpoints</span>
          </div>

          <div className="endpoints-list">
            <div className="endpoint-row">
              <span className="method get">GET</span>
              <code>/health</code>
              <span className="endpoint-name">Health check</span>
            </div>
            <div className="endpoint-row">
              <span className="method post">POST</span>
              <code>/api/v1/process</code>
              <span className="endpoint-name">Process document</span>
            </div>
            <div className="endpoint-row">
              <span className="method post">POST</span>
              <code>/api/v1/batch</code>
              <span className="endpoint-name">Batch process</span>
            </div>
            <div className="endpoint-row">
              <span className="method post">POST</span>
              <code>/api/v1/index/insert</code>
              <span className="endpoint-name">Insert record</span>
            </div>
            <div className="endpoint-row">
              <span className="method get">GET</span>
              <code>/api/v1/index/search</code>
              <span className="endpoint-name">Search index</span>
            </div>
            <div className="endpoint-row">
              <span className="method post">POST</span>
              <code>/api/v1/match</code>
              <span className="endpoint-name">Match documents</span>
            </div>
            <div className="endpoint-row">
              <span className="method post">POST</span>
              <code>/api/v1/compare</code>
              <span className="endpoint-name">Compare docs</span>
            </div>
          </div>

          <div className="server-features">
            {features.map((feature) => (
              <div key={feature.title} className="server-feature">
                <div className="feature-icon">
                  <feature.icon size={18} strokeWidth={1.5} />
                </div>
                <div className="feature-text">
                  <h4>{feature.title}</h4>
                  <p>{feature.desc}</p>
                </div>
              </div>
            ))}
          </div>
        </motion.div>
      </div>
    </section>
  )
}
