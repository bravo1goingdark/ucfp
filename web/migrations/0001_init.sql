-- ucfp_web v1 — users, sessions, API keys, usage events.
-- Apply locally:  pnpm wrangler d1 migrations apply ucfp_web --local
-- Apply remote:   pnpm wrangler d1 migrations apply ucfp_web --remote

CREATE TABLE IF NOT EXISTS users (
    id                  TEXT PRIMARY KEY,
    email               TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash       TEXT NOT NULL,
    tenant_id           INTEGER NOT NULL UNIQUE,
    created_at          INTEGER NOT NULL,
    email_verified_at   INTEGER
);

CREATE TABLE IF NOT EXISTS sessions (
    id                  TEXT PRIMARY KEY,             -- sha256(hex) of the cookie token
    user_id             TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at          INTEGER NOT NULL,
    created_at          INTEGER NOT NULL,
    user_agent          TEXT,
    ip                  TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_user   ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expiry ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS api_keys (
    id                  TEXT PRIMARY KEY,
    user_id             TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    prefix              TEXT NOT NULL,                -- e.g. "ucfp_3f9a"
    key_hash            TEXT NOT NULL UNIQUE,         -- sha256(hex) of the full secret
    rate_limit_per_min  INTEGER NOT NULL DEFAULT 600,
    daily_quota         INTEGER NOT NULL DEFAULT 50000,
    created_at          INTEGER NOT NULL,
    last_used_at        INTEGER,
    revoked_at          INTEGER
);
CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_id);

CREATE TABLE IF NOT EXISTS usage_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id             TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    api_key_id          TEXT REFERENCES api_keys(id) ON DELETE SET NULL,
    modality            TEXT NOT NULL CHECK (modality IN ('text', 'image', 'audio')),
    algorithm           TEXT,
    bytes_in            INTEGER NOT NULL DEFAULT 0,
    status              INTEGER NOT NULL,
    latency_ms          INTEGER NOT NULL DEFAULT 0,
    created_at          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_usage_user_time
    ON usage_events(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_usage_user_modality_day
    ON usage_events(user_id, modality, created_at);

-- Tenant 0 reserved for anonymous demo traffic; never linked to a user row.
-- The first real user gets tenant_id=1; the trigger below auto-allocates.
CREATE TRIGGER IF NOT EXISTS users_assign_tenant
AFTER INSERT ON users WHEN NEW.tenant_id IS NULL OR NEW.tenant_id = 0
BEGIN
    UPDATE users
       SET tenant_id = COALESCE((SELECT MAX(tenant_id) FROM users WHERE tenant_id > 0), 0) + 1
     WHERE id = NEW.id;
END;
