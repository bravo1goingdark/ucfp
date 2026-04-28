// Tweaks state — Svelte 5 runes module. Survives HMR via $state().
// Persisted to localStorage so the user's choices stick across reloads.

const STORAGE_KEY = 'ucfp.tweaks.v1';

const DEFAULTS = {
  theme: 'paper' as 'paper' | 'snow' | 'ink',
  accentHue: 130,
  density: 'cozy' as 'cozy' | 'default' | 'airy'
};

export type TweaksState = typeof DEFAULTS;

function load(): TweaksState {
  if (typeof localStorage === 'undefined') return { ...DEFAULTS };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULTS };
    const parsed = JSON.parse(raw);
    return { ...DEFAULTS, ...parsed };
  } catch {
    return { ...DEFAULTS };
  }
}

export const tweaks = $state<TweaksState>(load());

export function setTweak<K extends keyof TweaksState>(key: K, value: TweaksState[K]) {
  tweaks[key] = value;
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(tweaks));
  }
}
