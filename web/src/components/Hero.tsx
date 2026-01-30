import { motion } from 'framer-motion'
import { ArrowRight, Github, Fingerprint, FileText, Image, Music, Video, Box } from 'lucide-react'
import './Hero.css'

const floatingIcons = [
  { icon: FileText, delay: 0, x: -280, y: -150 },
  { icon: Image, delay: 0.5, x: 280, y: -140 },
  { icon: Music, delay: 1, x: -260, y: 180 },
  { icon: Video, delay: 1.5, x: 260, y: 190 },
  { icon: Fingerprint, delay: 2, x: -320, y: 20 },
  { icon: Box, delay: 2.5, x: 320, y: 40 },
]

export default function Hero() {
  return (
    <section className="hero">
      {/* Floating Animated Icons */}
      <div className="floating-icons-container">
        {floatingIcons.map((item, index) => (
          <motion.div
            key={index}
            className="floating-icon"
            style={{ left: `calc(50% + ${item.x}px)`, top: `calc(50% + ${item.y}px)` }}
            initial={{ opacity: 0, scale: 0 }}
            animate={{ 
              opacity: [0.2, 0.5, 0.2],
              scale: [1, 1.05, 1],
              rotate: [0, 5, 0]
            }}
            transition={{ 
              duration: 4,
              delay: item.delay,
              repeat: Infinity,
              ease: "easeInOut"
            }}
          >
            <item.icon size={28} strokeWidth={1.5} />
          </motion.div>
        ))}
      </div>

      <motion.span
        className="hero-badge"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
      >
        Open Source
      </motion.span>

      <motion.h1
        className="hero-title"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.1 }}
      >
        Universal Content
        <br />
        <span className="gradient-text">Fingerprinting</span>
      </motion.h1>

      <motion.p
        className="hero-subtitle"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.2 }}
      >
        Deterministic, reproducible content fingerprints for text, images, audio, 
        video, documents, and 3D models. Built in Rust for performance and safety.
      </motion.p>

      <motion.div
        className="hero-buttons"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.3 }}
      >
        <a href="https://github.com/bravo1goingdark/ucfp" className="btn btn-primary">
          <Github size={18} />
          Get Started
        </a>
        <a href="#features" className="btn btn-secondary">
          Learn More
          <ArrowRight size={16} />
        </a>
      </motion.div>

      <motion.div
        className="hero-stats"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.5, delay: 0.5 }}
      >
        <div className="stat">
          <span className="stat-value">~30ms</span>
          <span className="stat-label">per 1K words</span>
        </div>
        <div className="stat-divider" />
        <div className="stat">
          <span className="stat-value">6</span>
          <span className="stat-label">modalities</span>
        </div>
        <div className="stat-divider" />
        <div className="stat">
          <span className="stat-value">6</span>
          <span className="stat-label">pipeline stages</span>
        </div>
      </motion.div>
    </section>
  )
}
