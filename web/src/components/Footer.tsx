import { motion } from 'framer-motion'
import { Fingerprint, Github } from 'lucide-react'
import './Footer.css'

export default function Footer() {
  return (
    <footer className="footer">
      <div className="footer-container">
        <motion.div
          className="footer-content"
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <div className="footer-brand">
            <div className="footer-logo">
              <Fingerprint size={24} strokeWidth={1.5} />
              <span>UCFP</span>
            </div>
            <p>Universal Content Fingerprinting</p>
          </div>

          <div className="footer-links">
            <a href="#features">Features</a>
            <a href="#pipeline">Pipeline</a>
            <a href="#modalities">Modalities</a>
            <a href="https://github.com/bravo1goingdark/ucfp" target="_blank" rel="noopener noreferrer">
              <Github size={18} />
            </a>
          </div>
        </motion.div>
      </div>
    </footer>
  )
}
