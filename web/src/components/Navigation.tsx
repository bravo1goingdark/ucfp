import { motion } from 'framer-motion'
import { Fingerprint, Menu, X, Github, Sun, Moon } from 'lucide-react'
import { useState } from 'react'
import { NavLink, useLocation } from 'react-router-dom'

interface NavigationProps {
  scrolled: boolean
  theme: 'light' | 'dark'
  toggleTheme: () => void
}

const primaryLinks = [
  { to: '/', label: 'Home' },
  { to: '/playground', label: 'Playground' },
  { to: '/dashboard', label: 'Dashboard' },
]

const landingAnchors = [
  { href: '#pipeline', label: 'Pipeline' },
  { href: '#benefits', label: 'Benefits' },
  { href: '#status', label: 'Status' },
  { href: '#faq', label: 'FAQ' },
]

export default function Navigation({ scrolled, theme, toggleTheme }: NavigationProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const location = useLocation()
  const isLanding = location.pathname === '/'

  return (
    <motion.nav
      initial={{ y: -40, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
      className={`fixed top-0 inset-x-0 z-50 transition-all duration-300 ${
        scrolled
          ? 'bg-white/80 dark:bg-zinc-950/80 backdrop-blur-xl border-b border-zinc-200/60 dark:border-zinc-800/60'
          : 'bg-transparent border-b border-transparent'
      }`}
    >
      <div className="mx-auto max-w-6xl px-6 h-16 flex items-center justify-between">
        <NavLink
          to="/"
          className="flex items-center gap-2 text-zinc-900 dark:text-zinc-50 font-semibold tracking-tight"
        >
          <Fingerprint size={20} strokeWidth={1.75} className="text-accent-600 dark:text-accent-400" />
          <span>UCFP</span>
        </NavLink>

        <div className="hidden md:flex items-center gap-1">
          {primaryLinks.map((link) => (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.to === '/'}
              className={({ isActive }) =>
                `px-3 py-1.5 text-sm rounded-md transition-colors ${
                  isActive
                    ? 'text-zinc-900 dark:text-zinc-50 bg-zinc-100 dark:bg-zinc-900'
                    : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-50'
                }`
              }
            >
              {link.label}
            </NavLink>
          ))}
          {isLanding && landingAnchors.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="px-3 py-1.5 text-sm text-zinc-500 dark:text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors"
            >
              {link.label}
            </a>
          ))}
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={toggleTheme}
            aria-label={theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
            className="p-2 rounded-md text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-50 hover:bg-zinc-100 dark:hover:bg-zinc-900 transition-colors"
          >
            {theme === 'light' ? <Moon size={16} strokeWidth={1.75} /> : <Sun size={16} strokeWidth={1.75} />}
          </button>
          <a
            href="https://github.com/bravo1goingdark/ucfp"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="GitHub"
            className="p-2 rounded-md text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-50 hover:bg-zinc-100 dark:hover:bg-zinc-900 transition-colors"
          >
            <Github size={16} strokeWidth={1.75} />
          </a>
          <button
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            aria-label="Toggle menu"
            className="md:hidden p-2 rounded-md text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-900"
          >
            {mobileMenuOpen ? <X size={18} /> : <Menu size={18} />}
          </button>
        </div>
      </div>

      {mobileMenuOpen && (
        <div className="md:hidden border-t border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
          <div className="px-6 py-4 flex flex-col gap-1">
            {primaryLinks.map((link) => (
              <NavLink
                key={link.to}
                to={link.to}
                end={link.to === '/'}
                onClick={() => setMobileMenuOpen(false)}
                className={({ isActive }) =>
                  `px-3 py-2 text-sm rounded-md ${
                    isActive
                      ? 'text-zinc-900 dark:text-zinc-50 bg-zinc-100 dark:bg-zinc-900'
                      : 'text-zinc-600 dark:text-zinc-400'
                  }`
                }
              >
                {link.label}
              </NavLink>
            ))}
            {isLanding && landingAnchors.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setMobileMenuOpen(false)}
                className="px-3 py-2 text-sm text-zinc-500 dark:text-zinc-500"
              >
                {link.label}
              </a>
            ))}
          </div>
        </div>
      )}
    </motion.nav>
  )
}
