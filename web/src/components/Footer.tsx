import { Fingerprint, Github } from 'lucide-react'

export default function Footer() {
  return (
    <footer className="border-t border-zinc-200 dark:border-zinc-800/80 bg-white dark:bg-zinc-950">
      <div className="mx-auto max-w-6xl px-6 py-10 flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <Fingerprint size={18} strokeWidth={1.75} className="text-accent-600 dark:text-accent-400" />
          <span className="text-sm font-semibold text-zinc-900 dark:text-zinc-50">UCFP</span>
          <span className="text-sm text-zinc-500 dark:text-zinc-500">— Universal Content Fingerprinting</span>
        </div>

        <nav className="flex items-center gap-5 text-sm text-zinc-500 dark:text-zinc-500">
          <a href="#pipeline" className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors">Pipeline</a>
          <a href="#benefits" className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors">Benefits</a>
          <a href="#status" className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors">Status</a>
          <a href="#faq" className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors">FAQ</a>
          <a
            href="https://github.com/bravo1goingdark/ucfp"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="GitHub"
            className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors"
          >
            <Github size={16} />
          </a>
        </nav>
      </div>
      <div className="border-t border-zinc-100 dark:border-zinc-900">
        <div className="mx-auto max-w-6xl px-6 py-4 text-xs text-zinc-400 dark:text-zinc-600">
          © 2025 UCFP · Open source under Apache-2.0
        </div>
      </div>
    </footer>
  )
}
