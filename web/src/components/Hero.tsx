import { motion } from 'framer-motion'
import { ArrowRight, Github, GitCommit, Clock } from 'lucide-react'
import { useEffect, useState } from 'react'
import './Hero.css'

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

export default function Hero() {
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
            timeAgo: getTimeAgo(commitDate)
          })
        }
      })
      .catch(() => null)
  }, [])

  return (
    <section className="hero">
      <div className="hero-badges">
        <motion.div
          className="badge-group"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
        >
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
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.5, delay: 0.2 }}
              title={commit.message}
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
        </motion.div>
      </div>

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
        video, documents, and 3D models.         Built in Rust for performance and safety. Available as a library or standalone REST API server.
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
          <span className="stat-value">~10ms</span>
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
