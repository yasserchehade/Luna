from collections.abc import Iterator
from contextlib import contextmanager
from typing import Any
from uuid import UUID

import psycopg
from psycopg.rows import dict_row

from app.core.config import settings


def _connection_string() -> str:
    return settings.database_url.replace("postgresql+psycopg://", "postgresql://", 1)


@contextmanager
def get_connection() -> Iterator[psycopg.Connection[dict[str, Any]]]:
    with psycopg.connect(_connection_string(), row_factory=dict_row) as connection:
        yield connection


def get_default_workspace_id(connection: psycopg.Connection[dict[str, Any]]) -> UUID:
    with connection.cursor() as cursor:
        cursor.execute(
            """
            SELECT id
            FROM workspaces
            WHERE name = %s AND kind = %s
            ORDER BY created_at
            LIMIT 1
            """,
            ("Default Workspace", "personal"),
        )
        row = cursor.fetchone()
        if row:
            return row["id"]

        cursor.execute(
            """
            INSERT INTO workspaces (name, kind)
            VALUES (%s, %s)
            RETURNING id
            """,
            ("Default Workspace", "personal"),
        )
        created = cursor.fetchone()
        if created is None:
            raise RuntimeError("Could not create default workspace.")
        return created["id"]
