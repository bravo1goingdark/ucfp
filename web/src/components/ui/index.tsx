import { motion } from 'framer-motion'
import { Check, Copy, Eye, EyeOff, X } from 'lucide-react'
import { useState } from 'react'

export function CopyButton({ text, size = 14 }: { text: string; size?: number }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <button
      onClick={handleCopy}
      title={copied ? 'Copied' : 'Copy'}
      className="inline-flex items-center justify-center p-1.5 rounded-md text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-50 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
    >
      {copied ? <Check size={size} /> : <Copy size={size} />}
    </button>
  )
}

export function PasswordButton({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [show, setShow] = useState(false)

  return (
    <div className="relative">
      <input
        type={show ? 'text' : 'password'}
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder="Enter API key"
        className="w-full pr-9 px-3 py-2 text-sm bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500"
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200"
      >
        {show ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  )
}

export function ScoreGauge({ value, label, size = 80 }: { value: number; label: string; size?: number }) {
  const pct = Math.min(100, Math.max(0, value * 100))
  const radius = (size - 12) / 2
  const circumference = 2 * Math.PI * radius
  const offset = circumference - (pct / 100) * circumference

  const colorClass =
    pct >= 70 ? 'text-emerald-500'
    : pct >= 40 ? 'text-amber-500'
    : 'text-rose-500'

  return (
    <div className="flex flex-col items-center gap-2">
      <div className="relative">
        <svg width={size} height={size} className="-rotate-90">
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            strokeWidth="6"
            className="stroke-zinc-200 dark:stroke-zinc-800"
          />
          <motion.circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            strokeWidth="6"
            strokeLinecap="round"
            strokeDasharray={circumference}
            initial={{ strokeDashoffset: circumference }}
            animate={{ strokeDashoffset: offset }}
            transition={{ duration: 0.8, ease: 'easeOut' }}
            className={`stroke-current ${colorClass}`}
          />
        </svg>
        <div
          className={`absolute inset-0 flex items-center justify-center font-semibold ${colorClass}`}
          style={{ fontSize: size / 3.5 }}
        >
          {pct.toFixed(0)}
        </div>
      </div>
      <span className="text-xs font-medium text-zinc-500 dark:text-zinc-400">{label}</span>
    </div>
  )
}

export function StatusBadge({ connected }: { connected: boolean }) {
  return (
    <div
      className={`inline-flex items-center gap-2 px-2.5 py-1 rounded-full text-xs font-medium border ${
        connected
          ? 'border-emerald-500/20 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
          : 'border-rose-500/20 bg-rose-500/10 text-rose-600 dark:text-rose-400'
      }`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${
          connected ? 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)]' : 'bg-rose-500'
        }`}
      />
      {connected ? 'Connected' : 'Disconnected'}
    </div>
  )
}

export function EmptyState({ icon, title, description }: { icon: React.ReactNode; title: string; description: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="mb-4 p-3 rounded-full bg-zinc-100 dark:bg-zinc-900 text-zinc-400 dark:text-zinc-500">
        {icon}
      </div>
      <p className="text-sm font-medium text-zinc-700 dark:text-zinc-300">{title}</p>
      <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-500">{description}</p>
    </div>
  )
}

export function LoadingSkeleton({ lines = 3 }: { lines?: number }) {
  return (
    <div className="space-y-3 py-4">
      {Array.from({ length: lines }).map((_, i) => (
        <div
          key={i}
          className="h-4 rounded bg-zinc-100 dark:bg-zinc-900 animate-pulse"
          style={{ width: i === lines - 1 ? '60%' : '100%' }}
        />
      ))}
    </div>
  )
}

export function Toast({ message, type = 'error', onClose }: { message: string; type?: 'error' | 'success' | 'info'; onClose: () => void }) {
  const styles = {
    error: 'border-rose-500/30 bg-rose-500/10 text-rose-600 dark:text-rose-400',
    success: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
    info: 'border-accent-500/30 bg-accent-500/10 text-accent-600 dark:text-accent-400',
  }[type]

  return (
    <motion.div
      initial={{ opacity: 0, y: -16, x: '-50%' }}
      animate={{ opacity: 1, y: 0, x: '-50%' }}
      exit={{ opacity: 0, y: -16, x: '-50%' }}
      className={`fixed top-20 left-1/2 z-[10000] flex items-center gap-3 px-4 py-2.5 text-sm font-medium rounded-lg border backdrop-blur-md ${styles}`}
    >
      {message}
      <button onClick={onClose} className="opacity-60 hover:opacity-100">
        <X size={14} />
      </button>
    </motion.div>
  )
}

export function Badge({ children, color = 'gray' }: { children: React.ReactNode; color?: 'gray' | 'green' | 'amber' | 'red' | 'purple' }) {
  const colorMap = {
    gray: 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400',
    green: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
    amber: 'bg-amber-500/10 text-amber-600 dark:text-amber-400',
    red: 'bg-rose-500/10 text-rose-600 dark:text-rose-400',
    purple: 'bg-accent-500/10 text-accent-600 dark:text-accent-400',
  }
  return (
    <span className={`inline-flex items-center px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded ${colorMap[color]}`}>
      {children}
    </span>
  )
}

export function Card({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) {
  return (
    <div
      onClick={onClick}
      className={`rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-5 transition-colors ${
        onClick ? 'cursor-pointer hover:border-zinc-300 dark:hover:border-zinc-700' : ''
      }`}
    >
      {children}
    </div>
  )
}

export function Input({ value, onChange, placeholder, type = 'text' }: { value: string; onChange: (v: string) => void; placeholder?: string; type?: string }) {
  return (
    <input
      type={type}
      value={value}
      onChange={e => onChange(e.target.value)}
      placeholder={placeholder}
      className="w-full px-3 py-2 text-sm bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500"
    />
  )
}

export function Button({ children, onClick, variant = 'primary', disabled = false }: {
  children: React.ReactNode
  onClick?: () => void
  variant?: 'primary' | 'secondary' | 'danger'
  disabled?: boolean
}) {
  const variantClass = {
    primary: 'bg-accent-600 hover:bg-accent-700 text-white border-transparent',
    secondary: 'bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-200 border-zinc-200 dark:border-zinc-800 hover:bg-zinc-50 dark:hover:bg-zinc-800',
    danger: 'bg-white dark:bg-zinc-900 text-rose-600 dark:text-rose-400 border-rose-500/30 hover:bg-rose-50 dark:hover:bg-rose-950/30',
  }[variant]

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md border transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${variantClass}`}
    >
      {children}
    </button>
  )
}
