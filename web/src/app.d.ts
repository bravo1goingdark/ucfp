// Cloudflare bindings + auth state types.
// See https://kit.svelte.dev/docs/types#app and the wrangler.toml in this repo.

import type {
  D1Database,
  KVNamespace,
  R2Bucket,
  AnalyticsEngineDataset,
  ExecutionContext
} from '@cloudflare/workers-types';

declare global {
  namespace App {
    interface Platform {
      env?: {
        // ── D1 / KV / R2 / Analytics Engine bindings ────────────────────
        DB?: D1Database;
        RATE_LIMIT?: KVNamespace;
        ANALYTICS?: AnalyticsEngineDataset;
        FILES?: R2Bucket;
        // ── Secrets ─────────────────────────────────────────────────────
        UCFP_API_URL?: string;
        UCFP_API_TOKEN?: string;
        TURNSTILE_SECRET?: string;
        SESSION_SECRET?: string;
        // ── Public vars ─────────────────────────────────────────────────
        PUBLIC_SITE_NAME?: string;
        PUBLIC_TURNSTILE_SITE_KEY?: string;
      };
      context: ExecutionContext;
      caches: CacheStorage & { default: Cache };
    }

    interface Locals {
      user: SessionUser | null;
      session: SessionInfo | null;
    }

    interface PageData {
      seo?: SeoData;
    }
  }

  /** Session-bound user shape exposed to handlers + pages via `event.locals.user`. */
  interface SessionUser {
    id: string;
    email: string;
    tenantId: number;
  }

  interface SessionInfo {
    id: string;       // sha256 of the cookie token (matches D1 `sessions.id`)
    expiresAt: number; // unix seconds
  }

  interface SeoData {
    title?: string;
    description?: string;
    canonical?: string;
    ogImage?: string;
    noindex?: boolean;
  }
}

export {};
