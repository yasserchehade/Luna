-- Convenience copy of the initial Luna PostgreSQL schema.
-- The deployment-facing copy lives at infrastructure/postgres/schema.sql.
\i /docker-entrypoint-initdb.d/schema.sql
