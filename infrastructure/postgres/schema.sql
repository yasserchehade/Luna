CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('personal', 'family', 'landlord', 'business')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS workspace_memberships (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);

CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN ('upload', 'email')),
    original_filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    storage_provider TEXT NOT NULL DEFAULT 'local_folder',
    storage_path TEXT NOT NULL,
    cabinet_status TEXT NOT NULL DEFAULT 'unplanned' CHECK (cabinet_status IN ('unplanned', 'suggested', 'confirmed', 'filed', 'needs_review')),
    suggested_cabinet_path TEXT,
    confirmed_cabinet_path TEXT,
    sha256 TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS document_texts (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    text_content TEXT NOT NULL DEFAULT '',
    extraction_method TEXT NOT NULL,
    page_count INTEGER,
    character_count INTEGER NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    extracted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS bills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
    supplier_entity_id UUID,
    supplier TEXT NOT NULL,
    amount NUMERIC(12, 2),
    currency CHAR(3) NOT NULL DEFAULT 'AUD',
    due_date DATE,
    invoice_number TEXT,
    category TEXT,
    classification TEXT CHECK (classification IN ('personal', 'business', 'property')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'unpaid', 'paid', 'overdue', 'archived')),
    extraction_confidence NUMERIC(4, 3),
    review_status TEXT NOT NULL DEFAULT 'needs_review' CHECK (review_status IN ('not_required', 'needs_review', 'confirmed')),
    review_reasons JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS household_entities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    display_name TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE household_entities
    DROP CONSTRAINT IF EXISTS household_entities_entity_type_check;

CREATE TABLE IF NOT EXISTS entity_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source_entity_type TEXT NOT NULL,
    source_entity_id UUID NOT NULL,
    relationship_type TEXT NOT NULL,
    target_entity_type TEXT NOT NULL,
    target_entity_id UUID NOT NULL,
    provenance_document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
    confidence NUMERIC(4, 3),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS supplier_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    supplier_entity_id UUID REFERENCES household_entities(id) ON DELETE SET NULL,
    profile_key TEXT NOT NULL,
    supplier_name TEXT NOT NULL,
    aliases JSONB NOT NULL DEFAULT '[]'::jsonb,
    category TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'needs_review', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, profile_key)
);

CREATE TABLE IF NOT EXISTS supplier_template_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    supplier_profile_id UUID NOT NULL REFERENCES supplier_profiles(id) ON DELETE CASCADE,
    version_label TEXT NOT NULL DEFAULT 'observed',
    fingerprint TEXT NOT NULL,
    expected_anchors JSONB NOT NULL DEFAULT '[]'::jsonb,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'needs_review', 'archived')),
    first_seen_document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (supplier_profile_id, fingerprint)
);

CREATE TABLE IF NOT EXISTS document_template_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    supplier_profile_id UUID REFERENCES supplier_profiles(id) ON DELETE SET NULL,
    template_version_id UUID REFERENCES supplier_template_versions(id) ON DELETE SET NULL,
    fingerprint TEXT,
    matched_anchors JSONB NOT NULL DEFAULT '[]'::jsonb,
    missing_anchors JSONB NOT NULL DEFAULT '[]'::jsonb,
    confidence NUMERIC(4, 3),
    status TEXT NOT NULL CHECK (status IN ('known', 'unknown', 'changed', 'needs_review')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'dismissed', 'archived')),
    related_entity_type TEXT,
    related_entity_id UUID,
    due_date DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS reminders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    remind_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled' CHECK (status IN ('scheduled', 'sent', 'dismissed', 'archived')),
    related_entity_type TEXT,
    related_entity_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE bills
    ADD COLUMN IF NOT EXISTS supplier_entity_id UUID;

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS storage_provider TEXT NOT NULL DEFAULT 'local_folder';

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS cabinet_status TEXT NOT NULL DEFAULT 'unplanned';

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS suggested_cabinet_path TEXT;

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS confirmed_cabinet_path TEXT;

ALTER TABLE documents
    DROP CONSTRAINT IF EXISTS documents_cabinet_status_check;

ALTER TABLE documents
    ADD CONSTRAINT documents_cabinet_status_check
    CHECK (cabinet_status IN ('unplanned', 'suggested', 'confirmed', 'filed', 'needs_review'));

ALTER TABLE bills
    ADD COLUMN IF NOT EXISTS extraction_confidence NUMERIC(4, 3);

ALTER TABLE bills
    ADD COLUMN IF NOT EXISTS review_status TEXT NOT NULL DEFAULT 'needs_review';

ALTER TABLE bills
    ADD COLUMN IF NOT EXISTS review_reasons JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE bills
    DROP CONSTRAINT IF EXISTS bills_review_status_check;

ALTER TABLE bills
    ADD CONSTRAINT bills_review_status_check
    CHECK (review_status IN ('not_required', 'needs_review', 'confirmed'));

ALTER TABLE tasks
    DROP CONSTRAINT IF EXISTS tasks_status_check;

ALTER TABLE tasks
    ADD CONSTRAINT tasks_status_check
    CHECK (status IN ('open', 'done', 'dismissed', 'archived'));

ALTER TABLE reminders
    DROP CONSTRAINT IF EXISTS reminders_status_check;

ALTER TABLE reminders
    ADD CONSTRAINT reminders_status_check
    CHECK (status IN ('scheduled', 'sent', 'dismissed', 'archived'));

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'fk_bills_supplier_entity'
    ) THEN
        ALTER TABLE bills
            ADD CONSTRAINT fk_bills_supplier_entity
            FOREIGN KEY (supplier_entity_id)
            REFERENCES household_entities(id)
            ON DELETE SET NULL;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS extraction_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT,
    confidence NUMERIC(4, 3),
    output JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    entity_type TEXT,
    entity_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_bills_workspace_status ON bills(workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_bills_workspace_review_status ON bills(workspace_id, review_status);
CREATE INDEX IF NOT EXISTS idx_bills_due_date ON bills(due_date);
CREATE INDEX IF NOT EXISTS idx_documents_workspace ON documents(workspace_id);
CREATE INDEX IF NOT EXISTS idx_documents_cabinet_status ON documents(workspace_id, cabinet_status);
CREATE INDEX IF NOT EXISTS idx_documents_search
    ON documents USING GIN (
        to_tsvector(
            'english',
            COALESCE(original_filename, '')
                || ' '
                || COALESCE(suggested_cabinet_path, '')
                || ' '
                || COALESCE(confirmed_cabinet_path, '')
        )
    );
CREATE INDEX IF NOT EXISTS idx_document_texts_extracted_at ON document_texts(extracted_at);
CREATE INDEX IF NOT EXISTS idx_document_texts_search
    ON document_texts USING GIN (to_tsvector('english', text_content));
CREATE INDEX IF NOT EXISTS idx_household_entities_workspace_type ON household_entities(workspace_id, entity_type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_household_entities_unique_name
    ON household_entities(workspace_id, entity_type, lower(display_name));
CREATE INDEX IF NOT EXISTS idx_entity_relationships_source
    ON entity_relationships(workspace_id, source_entity_type, source_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_relationships_target
    ON entity_relationships(workspace_id, target_entity_type, target_entity_id);
CREATE INDEX IF NOT EXISTS idx_supplier_profiles_workspace_key
    ON supplier_profiles(workspace_id, profile_key);
CREATE INDEX IF NOT EXISTS idx_supplier_template_versions_profile
    ON supplier_template_versions(supplier_profile_id, status);
CREATE INDEX IF NOT EXISTS idx_document_template_matches_document
    ON document_template_matches(document_id);
CREATE INDEX IF NOT EXISTS idx_tasks_workspace_status ON tasks(workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_reminders_workspace_status ON reminders(workspace_id, status, remind_at);
CREATE INDEX IF NOT EXISTS idx_audit_events_workspace_created
    ON audit_events(workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_entity
    ON audit_events(workspace_id, entity_type, entity_id, created_at DESC);
