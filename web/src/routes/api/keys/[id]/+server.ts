// DELETE /api/keys/[id]   → 204     (soft revoke; scoped to current user)
//
// We never hard-delete: the FK on `usage_events.api_key_id` would null out
// the audit trail. Instead we set `revoked_at = now`. A row that's already
// revoked or doesn't belong to the caller responds 404 (we don't leak the
// existence of other users' keys).

import { error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { revokeKey } from '$lib/server/keys';

export const DELETE: RequestHandler = async ({ params, locals, platform }) => {
  if (!locals.user) error(401, 'unauthenticated');
  const env = platform?.env;
  if (!env?.DB) error(503, 'auth backend not configured');

  const id = params.id;
  if (!id) error(400, 'missing key id');

  const ok = await revokeKey(env.DB, { id, userId: locals.user.id });
  if (!ok) error(404, 'key not found');

  return new Response(null, { status: 204 });
};
