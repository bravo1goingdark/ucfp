import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'

export interface PerceptualConfig {
  version: number
  k: number
  w: number
  minhash_bands: number
  minhash_rows_per_band: number
  seed: number
  use_parallel: boolean
  include_intermediates: boolean
}

export interface SemanticConfig {
  tier: string
  mode: string
  model_name: string
  normalize: boolean
  device: string
  max_sequence_length: number
  enable_chunking: boolean
  chunk_overlap_ratio: number
  pooling_strategy: string
  api_url?: string
  api_auth_header?: string
  api_provider?: string
  api_timeout_secs?: number
  enable_resilience: boolean
}

export interface MatchDefaults {
  strategy: 'perceptual' | 'semantic' | 'hybrid'
  max_results: number
  oversample_factor: number
  min_score: number
}

export interface UcfpConfig {
  serverUrl: string
  apiKey: string
  defaultTenant: string
  enablePerceptual: boolean
  enableSemantic: boolean
  perceptual: PerceptualConfig
  semantic: SemanticConfig
  match: MatchDefaults
}

export const DEFAULT_PERCEPTUAL: PerceptualConfig = {
  version: 1,
  k: 9,
  w: 4,
  minhash_bands: 16,
  minhash_rows_per_band: 8,
  seed: 0xF00DBAAD,
  use_parallel: false,
  include_intermediates: true,
}

export const DEFAULT_SEMANTIC: SemanticConfig = {
  tier: 'balanced',
  mode: 'onnx',
  model_name: 'bge-small-en-v1.5',
  normalize: true,
  device: 'cpu',
  max_sequence_length: 512,
  enable_chunking: false,
  chunk_overlap_ratio: 0.5,
  pooling_strategy: 'weighted_mean',
  api_url: '',
  api_auth_header: '',
  api_provider: '',
  api_timeout_secs: 30,
  enable_resilience: true,
}

export const DEFAULT_MATCH: MatchDefaults = {
  strategy: 'hybrid',
  max_results: 10,
  oversample_factor: 1.5,
  min_score: 0.0,
}

const STORAGE_KEY = 'ucfp-config-v1'

function readEnv(name: string): string {
  const v = (import.meta.env as Record<string, string | undefined>)[name]
  return v ?? ''
}

function loadFromStorage(): UcfpConfig {
  const fallback: UcfpConfig = {
    serverUrl: readEnv('VITE_API_URL') || 'http://localhost:8080',
    apiKey: readEnv('VITE_API_KEY') || '',
    defaultTenant: 'default',
    enablePerceptual: true,
    enableSemantic: false,
    perceptual: { ...DEFAULT_PERCEPTUAL },
    semantic: { ...DEFAULT_SEMANTIC },
    match: { ...DEFAULT_MATCH },
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) {
      const legacyUrl = localStorage.getItem('ucfp-server-url')
      const legacyKey = localStorage.getItem('ucfp-api-key')
      if (legacyUrl) fallback.serverUrl = legacyUrl
      if (legacyKey) fallback.apiKey = legacyKey
      return fallback
    }
    const parsed = JSON.parse(raw) as Partial<UcfpConfig>
    return {
      ...fallback,
      ...parsed,
      perceptual: { ...fallback.perceptual, ...(parsed.perceptual || {}) },
      semantic: { ...fallback.semantic, ...(parsed.semantic || {}) },
      match: { ...fallback.match, ...(parsed.match || {}) },
    }
  } catch {
    return fallback
  }
}

interface ConfigContextValue {
  config: UcfpConfig
  setConfig: (updater: (prev: UcfpConfig) => UcfpConfig) => void
  update: <K extends keyof UcfpConfig>(key: K, value: UcfpConfig[K]) => void
  updatePerceptual: <K extends keyof PerceptualConfig>(key: K, value: PerceptualConfig[K]) => void
  updateSemantic: <K extends keyof SemanticConfig>(key: K, value: SemanticConfig[K]) => void
  updateMatch: <K extends keyof MatchDefaults>(key: K, value: MatchDefaults[K]) => void
  reset: () => void
}

const ConfigContext = createContext<ConfigContextValue | null>(null)

export function ConfigProvider({ children }: { children: ReactNode }) {
  const [config, setConfigState] = useState<UcfpConfig>(() => loadFromStorage())

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(config))
      localStorage.setItem('ucfp-server-url', config.serverUrl)
      localStorage.setItem('ucfp-api-key', config.apiKey)
    } catch {
      /* ignore quota */
    }
  }, [config])

  const value = useMemo<ConfigContextValue>(() => {
    const setConfig: ConfigContextValue['setConfig'] = (updater) => setConfigState(updater)
    return {
      config,
      setConfig,
      update: (key, value) => setConfigState(prev => ({ ...prev, [key]: value })),
      updatePerceptual: (key, value) =>
        setConfigState(prev => ({ ...prev, perceptual: { ...prev.perceptual, [key]: value } })),
      updateSemantic: (key, value) =>
        setConfigState(prev => ({ ...prev, semantic: { ...prev.semantic, [key]: value } })),
      updateMatch: (key, value) =>
        setConfigState(prev => ({ ...prev, match: { ...prev.match, [key]: value } })),
      reset: () =>
        setConfigState({
          serverUrl: readEnv('VITE_API_URL') || 'http://localhost:8080',
          apiKey: readEnv('VITE_API_KEY') || '',
          defaultTenant: 'default',
          enablePerceptual: true,
          enableSemantic: false,
          perceptual: { ...DEFAULT_PERCEPTUAL },
          semantic: { ...DEFAULT_SEMANTIC },
          match: { ...DEFAULT_MATCH },
        }),
    }
  }, [config])

  return <ConfigContext.Provider value={value}>{children}</ConfigContext.Provider>
}

export function useConfig(): ConfigContextValue {
  const ctx = useContext(ConfigContext)
  if (!ctx) throw new Error('useConfig must be used within a ConfigProvider')
  return ctx
}
