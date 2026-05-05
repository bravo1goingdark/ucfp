/**
 * Cross-browser fullscreen helper. Tries the Fullscreen API first; falls back
 * to an in-page `position: fixed` overlay when the API is unavailable (iframes,
 * sandboxed contexts, browsers that block `requestFullscreen`).
 */
export interface FullscreenController {
  readonly active: boolean;
  enter(target: HTMLElement): Promise<void>;
  exit(): Promise<void>;
  toggle(target: HTMLElement): Promise<void>;
}

type FsDocument = Document & {
  webkitFullscreenElement?: Element | null;
  msFullscreenElement?: Element | null;
  webkitExitFullscreen?: () => Promise<void>;
  msExitFullscreen?: () => Promise<void>;
};

type FsElement = HTMLElement & {
  webkitRequestFullscreen?: () => Promise<void>;
  msRequestFullscreen?: () => Promise<void>;
};

function getFsElement(doc: FsDocument): Element | null {
  return doc.fullscreenElement ?? doc.webkitFullscreenElement ?? doc.msFullscreenElement ?? null;
}

async function requestFs(el: FsElement): Promise<void> {
  if (el.requestFullscreen) return el.requestFullscreen();
  if (el.webkitRequestFullscreen) return el.webkitRequestFullscreen();
  if (el.msRequestFullscreen) return el.msRequestFullscreen();
  throw new Error('Fullscreen API not available');
}

async function exitFs(doc: FsDocument): Promise<void> {
  if (doc.exitFullscreen) return doc.exitFullscreen();
  if (doc.webkitExitFullscreen) return doc.webkitExitFullscreen();
  if (doc.msExitFullscreen) return doc.msExitFullscreen();
}

/**
 * Returns a controller plus a Svelte 5 `$state`-friendly snapshot. Caller
 * binds `controller.active` to a class to toggle in-page styles when the
 * native API isn't usable.
 */
export function createFullscreen(): FullscreenController & { destroy: () => void } {
  let activeRef: HTMLElement | null = null;
  let fallback = false;
  let active = $state(false);

  function syncFromBrowser() {
    if (typeof document === 'undefined') return;
    const fs = getFsElement(document as FsDocument);
    active = fallback ? !!activeRef : fs === activeRef;
    if (!active && activeRef && fallback) {
      activeRef.classList.remove('chart-fs-fallback');
      activeRef = null;
      fallback = false;
    }
  }

  function onKey(ev: KeyboardEvent) {
    if (ev.key === 'Escape' && fallback && activeRef) {
      void exit();
    }
  }

  if (typeof document !== 'undefined') {
    document.addEventListener('fullscreenchange', syncFromBrowser);
    document.addEventListener('webkitfullscreenchange', syncFromBrowser);
    document.addEventListener('msfullscreenchange', syncFromBrowser);
    document.addEventListener('keydown', onKey);
  }

  async function enter(target: HTMLElement) {
    activeRef = target;
    try {
      await requestFs(target as FsElement);
      fallback = false;
    } catch {
      // Fallback: full-viewport overlay class.
      target.classList.add('chart-fs-fallback');
      fallback = true;
      active = true;
    }
    syncFromBrowser();
  }

  async function exit() {
    if (fallback && activeRef) {
      activeRef.classList.remove('chart-fs-fallback');
      activeRef = null;
      fallback = false;
      active = false;
      return;
    }
    try {
      await exitFs(document as FsDocument);
    } catch {
      /* ignore */
    }
    activeRef = null;
    syncFromBrowser();
  }

  async function toggle(target: HTMLElement) {
    if (active) await exit();
    else await enter(target);
  }

  function destroy() {
    if (typeof document === 'undefined') return;
    document.removeEventListener('fullscreenchange', syncFromBrowser);
    document.removeEventListener('webkitfullscreenchange', syncFromBrowser);
    document.removeEventListener('msfullscreenchange', syncFromBrowser);
    document.removeEventListener('keydown', onKey);
  }

  return {
    get active() {
      return active;
    },
    enter,
    exit,
    toggle,
    destroy,
  };
}
