import type { Handle, HandleServerError } from '@sveltejs/kit';
import { sequence } from '@sveltejs/kit/hooks';
import { redirect } from '@sveltejs/kit';
import { readSessionCookie, clearSessionCookie } from '$lib/server/cookies';
import { validateSession } from '$lib/server/auth';

// ── Session resolution ──────────────────────────────────────────────────
// Touching `event.platform.env` throws inside SvelteKit's prerender
// pre-flight, so we bail out before any binding access when:
//   - there's no session cookie (no user to look up), OR
//   - we're being invoked during build-time prerender (no platform).
const handleSession: Handle = async ({ event, resolve }) => {
  event.locals.user = null;
  event.locals.session = null;

  // No cookie ⇒ no session ⇒ no need to touch any binding. This also
  // covers the prerender path because SvelteKit doesn't pass cookies
  // during build-time crawls.
  if (!event.cookies.get('ucfp_session')) return resolve(event);

  const env = event.platform?.env;
  if (!env || !env.DB || !env.RATE_LIMIT || !env.SESSION_SECRET) {
    return resolve(event);
  }

  const token = await readSessionCookie(event.cookies, env.SESSION_SECRET);
  if (!token) return resolve(event);

  const result = await validateSession(env.DB, env.RATE_LIMIT, token);
  if (!result) {
    clearSessionCookie(event.cookies);
    return resolve(event);
  }

  event.locals.user = {
    id: result.user.id,
    email: result.user.email,
    tenantId: result.user.tenant_id
  };
  event.locals.session = result.session;

  return resolve(event);
};

// ── Auth guard (path-prefix routing) ────────────────────────────────────
const handleAuthGuard: Handle = async ({ event, resolve }) => {
  const path = event.url.pathname;
  const user = event.locals.user;

  // Authenticated areas: redirect to /login when no session.
  const protectedPrefixes = ['/dashboard', '/api/keys', '/api/usage'];
  if (!user && protectedPrefixes.some((p) => path === p || path.startsWith(p + '/'))) {
    const next = encodeURIComponent(path + event.url.search);
    redirect(303, `/login?next=${next}`);
  }

  // Auth pages: send already-logged-in users to the dashboard.
  if (user && (path === '/login' || path === '/signup')) {
    redirect(303, '/dashboard');
  }

  return resolve(event);
};

// ── Security headers ────────────────────────────────────────────────────
const handleSecurity: Handle = async ({ event, resolve }) => {
  const response = await resolve(event);
  response.headers.set('X-Content-Type-Options', 'nosniff');
  response.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
  response.headers.set('X-Frame-Options', 'DENY');
  response.headers.set(
    'Permissions-Policy',
    'camera=(), microphone=(), geolocation=(), interest-cohort=()'
  );
  // CSP: SSR'd SvelteKit + spectrogram/PNG data URIs from upstream inspect.
  // `style-src 'unsafe-inline'` is required by Svelte's scoped style attrs;
  // `img-src data: blob:` lets the audio Spectrogram render its base64 PNGs.
  response.headers.set(
    'Content-Security-Policy',
    [
      "default-src 'self'",
      "img-src 'self' data: blob:",
      "style-src 'self' 'unsafe-inline'",
      "script-src 'self' 'unsafe-inline'",
      "connect-src 'self'",
      "font-src 'self' data:",
      "object-src 'none'",
      "base-uri 'self'",
      "frame-ancestors 'none'",
      "form-action 'self'"
    ].join('; ')
  );
  if (event.url.protocol === 'https:') {
    response.headers.set(
      'Strict-Transport-Security',
      'max-age=31536000; includeSubDomains'
    );
  }
  return response;
};

export const handle: Handle = sequence(handleSession, handleAuthGuard, handleSecurity);

export const handleError: HandleServerError = ({ error, event, status }) => {
  const id = crypto.randomUUID();
  console.error(`[${id}] ${event.url.pathname}:`, error);

  // Forward to the Workers Analytics Engine if the binding is present —
  // gives us a queryable error stream without spinning up a dedicated
  // error-monitoring SaaS. Failures here are best-effort (e.g. dataset
  // limits, missing binding); never let monitoring crash the request.
  try {
    const analytics = event.platform?.env?.ANALYTICS;
    if (analytics) {
      const err = error as { name?: string; message?: string; stack?: string };
      analytics.writeDataPoint({
        blobs: [
          'server-error',
          err?.name ?? 'Error',
          (err?.message ?? '').slice(0, 256),
          event.url.pathname.slice(0, 256),
          event.request.method,
          (err?.stack ?? '').slice(0, 1024),
          event.locals.user?.id ?? '',
        ],
        doubles: [status ?? 500],
        indexes: [id.slice(0, 32)],
      });
    }
  } catch {
    /* swallow */
  }
  return { message: 'Something went wrong on our end.', id };
};
