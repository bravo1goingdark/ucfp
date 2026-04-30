// Browser-only localStorage-backed bookmark list of fingerprints the
// user has saved. The Rust backend has no list endpoint, so this is
// the only way the Records page can show "your recent records".
//
// Capped at MAX entries; oldest evicted first. SSR-safe (no-op when
// `window` is undefined).

import type { RecordHistoryEntry } from '$lib/types/api';

const KEY = 'ucfp:records:v1';
const MAX = 200;

function read(): RecordHistoryEntry[] {
  if (typeof window === 'undefined') return [];
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RecordHistoryEntry[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function write(entries: RecordHistoryEntry[]): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(KEY, JSON.stringify(entries.slice(-MAX)));
  } catch {
    /* quota exceeded — silently drop */
  }
}

export interface RecordHistoryStore {
  readonly entries: RecordHistoryEntry[];
  add(e: RecordHistoryEntry): void;
  remove(recordId: string): void;
  clear(): void;
  refresh(): void;
}

export function createRecordHistory(): RecordHistoryStore {
  let entries = $state<RecordHistoryEntry[]>(read());

  return {
    get entries() {
      return entries;
    },
    add(e) {
      // De-dupe by recordId; new wins.
      const filtered = entries.filter((x) => x.recordId !== e.recordId);
      filtered.push(e);
      entries = filtered.slice(-MAX);
      write(entries);
    },
    remove(recordId) {
      entries = entries.filter((x) => x.recordId !== recordId);
      write(entries);
    },
    clear() {
      entries = [];
      write(entries);
    },
    refresh() {
      entries = read();
    }
  };
}
