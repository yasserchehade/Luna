from fastapi import APIRouter

from app.db import get_connection, get_default_workspace_id
from app.models.audit import AuditEvent

router = APIRouter(prefix="/audit-events", tags=["audit"])


@router.get("", response_model=list[AuditEvent])
def list_audit_events(limit: int = 30) -> list[AuditEvent]:
    bounded_limit = max(1, min(limit, 100))
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, event_type, entity_type, entity_id, metadata, created_at
                FROM audit_events
                WHERE workspace_id = %s
                ORDER BY created_at DESC
                LIMIT %s
                """,
                (workspace_id, bounded_limit),
            )
            rows = cursor.fetchall()

    return [
        AuditEvent(
            id=str(row["id"]),
            event_type=row["event_type"],
            entity_type=row["entity_type"],
            entity_id=str(row["entity_id"]) if row["entity_id"] else None,
            metadata=row["metadata"],
            created_at=row["created_at"],
        )
        for row in rows
    ]
