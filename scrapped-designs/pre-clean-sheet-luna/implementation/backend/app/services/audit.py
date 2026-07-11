from collections.abc import Mapping
from typing import Any

from psycopg import Cursor
from psycopg.types.json import Jsonb


def record_audit_event(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    event_type: str,
    entity_type: str | None = None,
    entity_id: object | None = None,
    metadata: Mapping[str, object] | None = None,
) -> None:
    cursor.execute(
        """
        INSERT INTO audit_events (
            workspace_id,
            event_type,
            entity_type,
            entity_id,
            metadata
        )
        VALUES (%s, %s, %s, %s, %s)
        """,
        (
            workspace_id,
            event_type,
            entity_type,
            entity_id,
            Jsonb(dict(metadata or {})),
        ),
    )
