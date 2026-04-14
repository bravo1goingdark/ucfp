import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import {
  ChevronDown, RotateCcw, Server, KeyRound, Sliders,
  Fingerprint, Brain, Search, Sparkles,
} from 'lucide-react'
import { useConfig } from '../context/ConfigContext'

function Section({
  title, icon, children, defaultOpen = false,
}: {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  const [open, setOpen] = useState(defaultOpen)
  return (
    <div className="border-b border-zinc-200 dark:border-zinc-800 last:border-b-0">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between py-3 text-left group"
      >
        <span className="flex items-center gap-2 text-sm font-medium text-zinc-700 dark:text-zinc-200">
          <span className="text-zinc-400 dark:text-zinc-500 group-hover:text-accent-600 dark:group-hover:text-accent-400">
            {icon}
          </span>
          {title}
        </span>
        <ChevronDown
          size={14}
          className={`text-zinc-400 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden"
          >
            <div className="pb-4 space-y-3">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

function FieldLabel({ children, hint }: { children: React.ReactNode; hint?: string }) {
  return (
    <label className="block text-xs font-medium text-zinc-600 dark:text-zinc-400 mb-1.5">
      {children}
      {hint && <span className="block mt-0.5 text-[10px] font-normal text-zinc-400 dark:text-zinc-500">{hint}</span>}
    </label>
  )
}

const inputClass =
  'w-full px-3 py-1.5 text-sm bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md text-zinc-900 dark:text-zinc-50 placeholder:text-zinc-400 focus:outline-none focus:ring-2 focus:ring-accent-500/30 focus:border-accent-500'

function NumberField({
  label, value, min, max, step = 1, onChange, hint,
}: {
  label: string
  value: number
  min?: number
  max?: number
  step?: number
  onChange: (v: number) => void
  hint?: string
}) {
  return (
    <div>
      <FieldLabel hint={hint}>{label}</FieldLabel>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={e => onChange(Number(e.target.value))}
        className={inputClass}
      />
    </div>
  )
}

function TextField({
  label, value, onChange, placeholder, hint, type = 'text',
}: {
  label: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  hint?: string
  type?: string
}) {
  return (
    <div>
      <FieldLabel hint={hint}>{label}</FieldLabel>
      <input
        type={type}
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className={inputClass}
      />
    </div>
  )
}

function SelectField({
  label, value, onChange, options, hint,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  options: { value: string; label: string }[]
  hint?: string
}) {
  return (
    <div>
      <FieldLabel hint={hint}>{label}</FieldLabel>
      <select value={value} onChange={e => onChange(e.target.value)} className={inputClass}>
        {options.map(o => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
    </div>
  )
}

function ToggleField({
  label, value, onChange, hint,
}: {
  label: string
  value: boolean
  onChange: (v: boolean) => void
  hint?: string
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer group">
      <button
        type="button"
        role="switch"
        aria-checked={value}
        onClick={() => onChange(!value)}
        className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors flex-shrink-0 mt-0.5 ${
          value ? 'bg-accent-600' : 'bg-zinc-300 dark:bg-zinc-700'
        }`}
      >
        <span
          className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
            value ? 'translate-x-5' : 'translate-x-1'
          }`}
        />
      </button>
      <div className="flex-1">
        <span className="text-sm text-zinc-700 dark:text-zinc-200">{label}</span>
        {hint && <p className="text-[10px] text-zinc-400 dark:text-zinc-500 mt-0.5">{hint}</p>}
      </div>
    </label>
  )
}

export default function ConfigPanel() {
  const { config, update, updatePerceptual, updateSemantic, updateMatch, reset } = useConfig()

  return (
    <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-5">
      <div className="flex items-center justify-between pb-3 border-b border-zinc-200 dark:border-zinc-800">
        <h3 className="flex items-center gap-2 text-sm font-semibold text-zinc-900 dark:text-zinc-50">
          <Sliders size={14} className="text-accent-600 dark:text-accent-400" />
          Configuration
        </h3>
        <button
          onClick={reset}
          className="flex items-center gap-1.5 text-xs text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-50 transition-colors"
        >
          <RotateCcw size={12} /> Reset
        </button>
      </div>

      <div className="mt-1">
        <Section title="Connection" icon={<Server size={14} />} defaultOpen>
          <TextField
            label="Server URL"
            value={config.serverUrl}
            onChange={v => update('serverUrl', v)}
            placeholder="http://localhost:8080"
            hint="UCFP backend base URL"
          />
          <div>
            <FieldLabel hint="Sent as X-API-Key header">
              <span className="inline-flex items-center gap-1">API Key <KeyRound size={10} className="opacity-60" /></span>
            </FieldLabel>
            <input
              type="password"
              value={config.apiKey}
              onChange={e => update('apiKey', e.target.value)}
              placeholder="optional"
              className={inputClass}
            />
          </div>
          <TextField
            label="Default Tenant"
            value={config.defaultTenant}
            onChange={v => update('defaultTenant', v)}
            placeholder="default"
          />
        </Section>

        <Section title="Pipeline Stages" icon={<Sparkles size={14} />} defaultOpen>
          <ToggleField
            label="Perceptual (MinHash LSH)"
            value={config.enablePerceptual}
            onChange={v => update('enablePerceptual', v)}
            hint="~82μs per 1K words"
          />
          <ToggleField
            label="Semantic (embeddings)"
            value={config.enableSemantic}
            onChange={v => update('enableSemantic', v)}
            hint="~1.1ms per 1K words (local ONNX)"
          />
        </Section>

        <Section title="Perceptual Config" icon={<Fingerprint size={14} />}>
          <NumberField label="Shingle size (k)" value={config.perceptual.k} min={1} max={32} onChange={v => updatePerceptual('k', v)} hint="Tokens per shingle. Larger = more context." />
          <NumberField label="Winnow window (w)" value={config.perceptual.w} min={1} max={32} onChange={v => updatePerceptual('w', v)} hint="Larger = fewer shingles, faster." />
          <NumberField label="MinHash bands" value={config.perceptual.minhash_bands} min={1} max={64} onChange={v => updatePerceptual('minhash_bands', v)} hint="More bands = higher recall." />
          <NumberField label="Rows per band" value={config.perceptual.minhash_rows_per_band} min={1} max={64} onChange={v => updatePerceptual('minhash_rows_per_band', v)} hint="More rows = higher precision." />
          <NumberField label="Seed" value={config.perceptual.seed} onChange={v => updatePerceptual('seed', v)} hint="Deterministic hashing." />
          <ToggleField label="Parallel MinHash" value={config.perceptual.use_parallel} onChange={v => updatePerceptual('use_parallel', v)} />
          <ToggleField label="Include intermediates" value={config.perceptual.include_intermediates} onChange={v => updatePerceptual('include_intermediates', v)} hint="Debug-only — larger response." />
        </Section>

        <Section title="Semantic Config" icon={<Brain size={14} />}>
          <SelectField
            label="Tier"
            value={config.semantic.tier}
            onChange={v => updateSemantic('tier', v)}
            options={[
              { value: 'fast', label: 'Fast (stub)' },
              { value: 'balanced', label: 'Balanced' },
              { value: 'accurate', label: 'Accurate' },
            ]}
          />
          <SelectField
            label="Mode"
            value={config.semantic.mode}
            onChange={v => updateSemantic('mode', v)}
            options={[
              { value: 'onnx', label: 'ONNX (local)' },
              { value: 'api', label: 'API (remote)' },
              { value: 'fast', label: 'Fast (deterministic)' },
            ]}
          />
          <TextField label="Model name" value={config.semantic.model_name} onChange={v => updateSemantic('model_name', v)} placeholder="bge-small-en-v1.5" />
          <NumberField label="Max sequence length" value={config.semantic.max_sequence_length} min={64} max={8192} step={64} onChange={v => updateSemantic('max_sequence_length', v)} />
          <ToggleField label="Normalize vectors" value={config.semantic.normalize} onChange={v => updateSemantic('normalize', v)} hint="Required for cosine similarity." />
          <ToggleField label="Enable chunking" value={config.semantic.enable_chunking} onChange={v => updateSemantic('enable_chunking', v)} hint="Split long texts into overlapping windows." />
          {config.semantic.enable_chunking && (
            <>
              <NumberField label="Chunk overlap ratio" value={config.semantic.chunk_overlap_ratio} min={0} max={0.9} step={0.05} onChange={v => updateSemantic('chunk_overlap_ratio', v)} />
              <SelectField
                label="Pooling strategy"
                value={config.semantic.pooling_strategy}
                onChange={v => updateSemantic('pooling_strategy', v)}
                options={[
                  { value: 'mean', label: 'Mean' },
                  { value: 'weighted_mean', label: 'Weighted mean' },
                  { value: 'max', label: 'Max' },
                  { value: 'first', label: 'First chunk' },
                ]}
              />
            </>
          )}
          {config.semantic.mode === 'api' && (
            <>
              <TextField label="API URL" value={config.semantic.api_url || ''} onChange={v => updateSemantic('api_url', v)} placeholder="https://..." />
              <TextField label="API Auth header" value={config.semantic.api_auth_header || ''} onChange={v => updateSemantic('api_auth_header', v)} placeholder="Bearer ..." />
              <SelectField
                label="API provider"
                value={config.semantic.api_provider || ''}
                onChange={v => updateSemantic('api_provider', v)}
                options={[
                  { value: '', label: 'Custom' },
                  { value: 'hf', label: 'Hugging Face' },
                  { value: 'openai', label: 'OpenAI' },
                ]}
              />
              <NumberField label="API timeout (s)" value={config.semantic.api_timeout_secs || 30} min={1} max={600} onChange={v => updateSemantic('api_timeout_secs', v)} />
            </>
          )}
          <ToggleField label="Resilience (retry + circuit breaker)" value={config.semantic.enable_resilience} onChange={v => updateSemantic('enable_resilience', v)} />
        </Section>

        <Section title="Match Defaults" icon={<Search size={14} />}>
          <SelectField
            label="Strategy"
            value={config.match.strategy}
            onChange={v => updateMatch('strategy', v as 'perceptual' | 'semantic' | 'hybrid')}
            options={[
              { value: 'perceptual', label: 'Perceptual (Jaccard)' },
              { value: 'semantic', label: 'Semantic (cosine)' },
              { value: 'hybrid', label: 'Hybrid' },
            ]}
          />
          <NumberField label="Max results" value={config.match.max_results} min={1} max={100} onChange={v => updateMatch('max_results', v)} />
          <NumberField label="Oversample factor" value={config.match.oversample_factor} min={1} max={10} step={0.1} onChange={v => updateMatch('oversample_factor', v)} hint="Semantic: query more candidates than needed." />
          <NumberField label="Min score" value={config.match.min_score} min={0} max={1} step={0.05} onChange={v => updateMatch('min_score', v)} />
        </Section>
      </div>
    </div>
  )
}
