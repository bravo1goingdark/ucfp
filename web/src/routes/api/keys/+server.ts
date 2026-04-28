// GET  /api/keys      → KeyRow[]              (session-auth)
// POST /api/keys      → CreatedKey + 201      (session-auth, plaintext token once)
//
// `hooks.server.ts` already redirects unauthenticated requests to /login (303).
// We still 401 explicitly so JSON clients (curl, the dashboard's `fetch`) get
// a typed error instead of HTML.

import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { createApiKey, listKeys } from '$lib/server/keys';

const NAME_MAX = 80;

export const GET: RequestHandler = async ({ locals, platform }) => {
  if (!locals.user) error(401, 'unauthenticated');
  const env = platform?.env;
  if (!env?.DB) error(503, 'auth backend not configured');

  const rows = await listKeys(env.DB, locals.user.id);
  return json(rows);
};

export const POST: RequestHandler = async ({ locals, platform, request }) => {
  if (!locals.user) error(401, 'unauthenticated');
  const env = platform?.env;
  if (!env?.DB) error(503, 'auth backend not configured');

  const body = await request.json().catch(() => null);
  if (!body || typeof body !== 'object') error(400, 'invalid json body');

  const rawName = (body as Record<string, unknown>).name;
  const name = typeof rawName === 'string' ? rawName.trim() : '';
  if (!name) error(400, 'name is required');
  if (name.length > NAME_MAX) error(400, `name must be ≤ ${NAME_MAX} characters`);

  const created = await createApiKey({
    db: env.DB,
    userId: locals.user.id,
    name
  });
  return json(created, { status: 201 });
};
