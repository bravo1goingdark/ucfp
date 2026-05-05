// POST /api/_errors — sink for client-side error reports.
// Writes to the Workers Analytics Engine dataset bound at `ANALYTICS`
// so server- and client-side errors share one query surface.

import type { RequestHandler } from './$types';

interface Payload {
  id?: string;
  url?: string;
  message?: string;
  stack?: string;
  ua?: string;
}

export const POST: RequestHandler = async ({ request, platform, getClientAddress, locals }) => {
  let body: Payload = {};
  try {
    body = (await request.json()) as Payload;
  } catch {
    /* keep empty */
  }
  const analytics = platform?.env?.ANALYTICS;
  if (analytics) {
    try {
      analytics.writeDataPoint({
        blobs: [
          'client-error',
          body.message?.slice(0, 256) ?? '',
          (body.url ?? '').slice(0, 256),
          (body.ua ?? '').slice(0, 200),
          (body.stack ?? '').slice(0, 1024),
          locals.user?.id ?? '',
          // Don't block on failed reverse-IP lookups; getClientAddress
          // is cheap because Cloudflare populates CF-Connecting-IP for us.
          (() => {
            try {
              return getClientAddress();
            } catch {
              return '';
            }
          })(),
        ],
        indexes: [(body.id ?? '').slice(0, 32)],
      });
    } catch {
      /* never crash on analytics failure */
    }
  }
  return new Response(null, { status: 204 });
};
