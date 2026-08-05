PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS conversation_messages (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL,
    conversation_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('member', 'luna')),
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    contextual_work_ids TEXT NOT NULL DEFAULT '[]',
    source_reference TEXT
);

CREATE INDEX IF NOT EXISTS conversation_messages_household
    ON conversation_messages(household_id, conversation_id, created_at, id);

CREATE TABLE IF NOT EXISTS household_work (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS household_work_household
    ON household_work(household_id, updated_at, id);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    storage_key TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS sources_household
    ON sources(household_id, created_at, id);

CREATE TABLE IF NOT EXISTS audit_events (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL,
    work_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(work_id, sequence)
);
