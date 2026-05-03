---
title: Authentication
order: 2
category: Get started
description: Bearer tokens, the X-Api-Key fallback, and how to rotate keys safely.
---

# Authentication

Every authenticated request carries a key. UCFP accepts two transports.

## Bearer token (preferred)

```http
Authorization: Bearer ucfp_8z2yQk9r3M4nP6vW8xC1aB5dE7fG2hJ4kL6mN8pQ
```

This is the canonical form. Use it for server-to-server calls and CLI scripts.

## X-Api-Key (fallback)

Some clients can not set arbitrary `Authorization` headers (browser fetch under restrictive CORS, certain webhook providers, locked-down corporate proxies). For those, send:

```http
X-Api-Key: ucfp_8z2yQk9r3M4nP6vW8xC1aB5dE7fG2hJ4kL6mN8pQ
```

The two transports are equivalent. If both are present, `Authorization: Bearer …` wins.

## Key shape

- Prefix `ucfp_`
- Body: 32 random bytes, base64url-encoded (no padding, ~43 chars)
- Total: 48 characters

The first 8 characters after the prefix (e.g. `ucfp_8z2yQk9r`) are stored as the **key prefix** so the dashboard can identify a key in lists without ever holding the secret. The full token is hashed with SHA-256 and only the digest is persisted server-side.

## Rotation

Keys do not expire by default. Rotate any key whose holder changed, whose machine was lost, or that was logged in plaintext.

Procedure:

1. Dashboard → **Keys → New key**. Note the new prefix.
2. Roll the new token through your callers. Verify they are using the new prefix in `/dashboard/usage`.
3. Dashboard → **Keys**, click the old key, **Revoke**.

Revoked keys reject with `401` immediately. They keep showing up in the list (greyed) so historical usage still resolves a key name.

## Tenant isolation

Each user has exactly one `tenant_id`, allocated at signup. Records are stored under that tenant; cross-tenant queries return 0 hits even on identical input. There is no API to share a tenant across users in v1.

## Service token (self-host / proxy mode)

When the SvelteKit Worker forwards calls to the Rust upstream, it uses a single service-level bearer (`UCFP_API_TOKEN`) plus an `X-Ucfp-Tenant: <id>` header. End-user keys never reach the Rust process. Self-hosters running the Rust binary directly use the single `UCFP_TOKEN` instead.

## What to never do

- Do not commit keys to git. Use environment variables or a secret manager.
- Do not embed a key in a single-page app. The browser is a public surface; use the demo path instead and let it allocate tenant 0 on the fly.
- Do not log the `Authorization` header. Log the prefix only; the dashboard correlates by prefix.
