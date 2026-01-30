import { motion } from 'framer-motion'
import { Fingerprint, Menu, X, Github, Sun, Moon } from 'lucide-react'
import { useState } from 'react'
import './Navigation.css'

interface NavigationProps {
  scrolled: boolean
  theme: 'light' | 'dark'
  toggleTheme: () => void
}

export default function Navigation({ scrolled, theme, toggleTheme }: NavigationProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)

  const navLinks = [
    { href: '#features', label: 'Features' },
    { href: '#pipeline', label: 'Pipeline' },
    { href: '#modalities', label: 'Modalities' },
  ]

  return (
    <motion.nav
      className={`navigation ${scrolled ? 'scrolled' : ''}`}
      initial={{ y: -100 }}
      animate={{ y: 0 }}
      transition={{ duration: 0.5 }}
    >
      <div className="nav-container">
        <a href="#" className="nav-logo">
          <Fingerprint size={22} strokeWidth={1.5} />
          <span>UCFP</span>
        </a>

        <div className="nav-links">
          {navLinks.map((link) => (
            <a key={link.href} href={link.href} className="nav-link">
              {link.label}
            </a>
          ))}
        </div>

        <div className="nav-actions">
          <button
            className="theme-toggle"
            onClick={toggleTheme}
            aria-label={theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
          >
            {theme === 'light' ? (
              <Moon size={18} strokeWidth={1.5} />
            ) : (
              <Sun size={18} strokeWidth={1.5} />
            )}
          </button>
          <a
            href="https://github.com/bravo1goingdark/ucfp"
            target="_blank"
            rel="noopener noreferrer"
            className="nav-github"
          >
            <Github size={20} strokeWidth={1.5} />
          </a>
          <button
            className="mobile-menu-btn"
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
          >
            {mobileMenuOpen ? <X size={20} /> : <Menu size={20} />}
          </button>
        </div>
      </div>

      {mobileMenuOpen && (
        <div className="mobile-menu">
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="mobile-nav-link"
              onClick={() => setMobileMenuOpen(false)}
            >
              {link.label}
            </a>
          ))}
        </div>
      )}
    </motion.nav>
  )
}
