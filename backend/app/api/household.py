from fastapi import APIRouter, HTTPException, status
from psycopg.types.json import Jsonb

from app.db import get_connection, get_default_workspace_id
from app.models.household import (
    EntityRelationship,
    EntityRelationshipActionResponse,
    EntityRelationshipCreate,
    EntityRelationshipDeleteResponse,
    EntityRelationshipsForEntity,
    GraphSuggestionActionResponse,
    GraphSuggestionList,
    HouseholdEntityActionResponse,
    HouseholdEntityCreate,
    HouseholdEntityUpdate,
    HouseholdEntity,
    HouseholdGraphNode,
    HouseholdGraph,
    HouseholdSummary,
    Reminder,
    ReminderActionResponse,
    ReminderCreate,
    ReminderStatus,
    Task,
    TaskActionResponse,
    TaskCreate,
    TaskStatus,
)
from app.services.audit import record_audit_event
from app.services.graph_suggestions import (
    accept_graph_suggestion,
    list_pending_graph_suggestions,
    reject_graph_suggestion,
)

router = APIRouter(prefix="/household", tags=["household"])


def _normalize_kind(value: str) -> str:
    return value.strip().lower().replace(" ", "_")


def _entity_from_row(row) -> HouseholdEntity:
    return HouseholdEntity(
        id=str(row["id"]),
        entity_type=row["entity_type"],
        display_name=row["display_name"],
        metadata=row["metadata"],
    )


def _graph_node_from_entity(row) -> HouseholdGraphNode:
    return HouseholdGraphNode(
        id=str(row["id"]),
        node_type=row["entity_type"],
        display_name=row["display_name"],
        metadata=row["metadata"],
    )


def _relationship_from_row(row) -> EntityRelationship:
    return EntityRelationship(
        id=str(row["id"]),
        source_entity_type=row["source_entity_type"],
        source_entity_id=str(row["source_entity_id"]),
        relationship_type=row["relationship_type"],
        target_entity_type=row["target_entity_type"],
        target_entity_id=str(row["target_entity_id"]),
        provenance_document_id=str(row["provenance_document_id"])
        if row["provenance_document_id"]
        else None,
        confidence=float(row["confidence"]) if row["confidence"] is not None else None,
    )


def _resolve_relationship_node(
    cursor,
    *,
    workspace_id,
    requested_entity_type: str | None,
    entity_id: str,
    side: str,
) -> str:
    normalized_type = (
        _normalize_kind(requested_entity_type)
        if requested_entity_type is not None
        else None
    )

    if normalized_type in {None, "entity", "household_entity"}:
        cursor.execute(
            """
            SELECT entity_type
            FROM household_entities
            WHERE workspace_id = %s
                AND id = %s
            """,
            (workspace_id, entity_id),
        )
        row = cursor.fetchone()
        if row is not None:
            return row["entity_type"]
        if normalized_type is not None:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"{side.capitalize()} household entity must exist.",
            )

    if normalized_type == "document":
        cursor.execute(
            """
            SELECT id
            FROM documents
            WHERE workspace_id = %s
                AND id = %s
            """,
            (workspace_id, entity_id),
        )
        if cursor.fetchone() is not None:
            return "document"

    if normalized_type == "bill":
        cursor.execute(
            """
            SELECT id
            FROM bills
            WHERE workspace_id = %s
                AND id = %s
            """,
            (workspace_id, entity_id),
        )
        if cursor.fetchone() is not None:
            return "bill"

    if normalized_type not in {None, "document", "bill"}:
        cursor.execute(
            """
            SELECT entity_type
            FROM household_entities
            WHERE workspace_id = %s
                AND id = %s
                AND entity_type = %s
            """,
            (workspace_id, entity_id, normalized_type),
        )
        row = cursor.fetchone()
        if row is not None:
            return row["entity_type"]

    raise HTTPException(
        status_code=status.HTTP_400_BAD_REQUEST,
        detail=f"{side.capitalize()} node must exist in the household graph.",
    )


def _task_from_row(row) -> Task:
    return Task(
        id=str(row["id"]),
        title=row["title"],
        description=row["description"],
        status=TaskStatus(row["status"]),
        due_date=row["due_date"].isoformat() if row["due_date"] else None,
        related_entity_type=row["related_entity_type"],
        related_entity_id=str(row["related_entity_id"])
        if row["related_entity_id"]
        else None,
    )


def _reminder_from_row(row) -> Reminder:
    return Reminder(
        id=str(row["id"]),
        title=row["title"],
        remind_at=row["remind_at"].isoformat(),
        status=ReminderStatus(row["status"]),
        related_entity_type=row["related_entity_type"],
        related_entity_id=str(row["related_entity_id"])
        if row["related_entity_id"]
        else None,
    )


RELATIONSHIP_SELECT = """
    SELECT
        id,
        source_entity_type,
        source_entity_id,
        relationship_type,
        target_entity_type,
        target_entity_id,
        provenance_document_id,
        confidence
    FROM entity_relationships
"""


@router.get("/entities", response_model=list[HouseholdEntity])
def list_entities() -> list[HouseholdEntity]:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, entity_type, display_name, metadata
                FROM household_entities
                ORDER BY entity_type, display_name
                """
            )
            rows = cursor.fetchall()

    return [_entity_from_row(row) for row in rows]


@router.post(
    "/entities",
    response_model=HouseholdEntityActionResponse,
    status_code=status.HTTP_201_CREATED,
)
def create_entity(request: HouseholdEntityCreate) -> HouseholdEntityActionResponse:
    entity_type = _normalize_kind(request.entity_type)

    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, entity_type, display_name, metadata
                FROM household_entities
                WHERE workspace_id = %s
                    AND entity_type = %s
                    AND lower(display_name) = lower(%s)
                LIMIT 1
                """,
                (workspace_id, entity_type, request.display_name),
            )
            row = cursor.fetchone()
            if row is None:
                cursor.execute(
                    """
                    INSERT INTO household_entities (
                        workspace_id,
                        entity_type,
                        display_name,
                        metadata
                    )
                    VALUES (%s, %s, %s, %s)
                    RETURNING id, entity_type, display_name, metadata
                    """,
                    (
                        workspace_id,
                        entity_type,
                        request.display_name.strip(),
                        Jsonb(request.metadata),
                    ),
                )
                row = cursor.fetchone()
                if row is not None:
                    record_audit_event(
                        cursor,
                        workspace_id=workspace_id,
                        event_type="household_entity.created",
                        entity_type=row["entity_type"],
                        entity_id=row["id"],
                        metadata={
                            "display_name": row["display_name"],
                            "metadata_keys": list((row["metadata"] or {}).keys()),
                        },
                    )

    if row is None:
        raise RuntimeError("Entity create did not return a saved entity.")
    return HouseholdEntityActionResponse(entity=_entity_from_row(row))


@router.get(
    "/entities/{entity_id}/relationships",
    response_model=EntityRelationshipsForEntity,
)
def entity_relationships(entity_id: str) -> EntityRelationshipsForEntity:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id
                FROM household_entities
                WHERE workspace_id = %s
                    AND id = %s
                """,
                (workspace_id, entity_id),
            )
            if cursor.fetchone() is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Household entity not found.",
                )

            cursor.execute(
                RELATIONSHIP_SELECT
                + """
                WHERE workspace_id = %s
                    AND (source_entity_id = %s OR target_entity_id = %s)
                ORDER BY created_at
                """,
                (workspace_id, entity_id, entity_id),
            )
            rows = cursor.fetchall()

    return EntityRelationshipsForEntity(
        entity_id=entity_id,
        relationships=[_relationship_from_row(row) for row in rows],
    )


@router.patch("/entities/{entity_id}", response_model=HouseholdEntityActionResponse)
def update_entity(
    entity_id: str,
    request: HouseholdEntityUpdate,
) -> HouseholdEntityActionResponse:
    fields = request.model_dump(exclude_unset=True)
    if not fields:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="At least one entity field must be provided.",
        )

    entity_type = (
        _normalize_kind(fields["entity_type"])
        if fields.get("entity_type") is not None
        else None
    )
    display_name = (
        fields["display_name"].strip()
        if fields.get("display_name") is not None
        else None
    )
    metadata = Jsonb(fields["metadata"]) if "metadata" in fields else None

    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            if entity_type is not None or display_name is not None:
                cursor.execute(
                    """
                    SELECT
                        COALESCE(%s, entity_type) AS next_entity_type,
                        COALESCE(%s, display_name) AS next_display_name
                    FROM household_entities
                    WHERE workspace_id = %s
                        AND id = %s
                    """,
                    (entity_type, display_name, workspace_id, entity_id),
                )
                next_identity = cursor.fetchone()
                if next_identity is None:
                    raise HTTPException(
                        status_code=status.HTTP_404_NOT_FOUND,
                        detail="Household entity not found.",
                    )

                cursor.execute(
                    """
                    SELECT id
                    FROM household_entities
                    WHERE workspace_id = %s
                        AND entity_type = %s
                        AND lower(display_name) = lower(%s)
                        AND id <> %s
                    LIMIT 1
                    """,
                    (
                        workspace_id,
                        next_identity["next_entity_type"],
                        next_identity["next_display_name"],
                        entity_id,
                    ),
                )
                if cursor.fetchone() is not None:
                    raise HTTPException(
                        status_code=status.HTTP_409_CONFLICT,
                        detail="A household entity with that type and name already exists.",
                    )

            cursor.execute(
                """
                UPDATE household_entities
                SET
                    entity_type = COALESCE(%s, entity_type),
                    display_name = COALESCE(%s, display_name),
                    metadata = COALESCE(%s, metadata),
                    updated_at = now()
                WHERE workspace_id = %s
                    AND id = %s
                RETURNING id, entity_type, display_name, metadata
                """,
                (entity_type, display_name, metadata, workspace_id, entity_id),
            )
            row = cursor.fetchone()
            if row is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Household entity not found.",
                )
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="household_entity.updated",
                entity_type=row["entity_type"],
                entity_id=row["id"],
                metadata={
                    "updated_fields": sorted(fields.keys()),
                    "display_name": row["display_name"],
                },
            )

            if entity_type is not None:
                cursor.execute(
                    """
                    UPDATE entity_relationships
                    SET source_entity_type = %s
                    WHERE workspace_id = %s
                        AND source_entity_id = %s
                    """,
                    (entity_type, workspace_id, entity_id),
                )
                cursor.execute(
                    """
                    UPDATE entity_relationships
                    SET target_entity_type = %s
                    WHERE workspace_id = %s
                        AND target_entity_id = %s
                    """,
                    (entity_type, workspace_id, entity_id),
                )

    return HouseholdEntityActionResponse(entity=_entity_from_row(row))


@router.get("/graph", response_model=HouseholdGraph)
def household_graph() -> HouseholdGraph:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, entity_type, display_name, metadata
                FROM household_entities
                WHERE workspace_id = %s
                ORDER BY created_at
                """,
                (workspace_id,),
            )
            entity_rows = cursor.fetchall()

            cursor.execute(
                """
                SELECT
                    id,
                    source_entity_type,
                    source_entity_id,
                    relationship_type,
                    target_entity_type,
                    target_entity_id,
                    provenance_document_id,
                    confidence
                FROM entity_relationships
                WHERE workspace_id = %s
                ORDER BY created_at
                """,
                (workspace_id,),
            )
            relationship_rows = cursor.fetchall()

            document_ids = {
                str(row[key])
                for row in relationship_rows
                for key in ("source_entity_id", "target_entity_id")
                if row[f"{key.split('_')[0]}_entity_type"] == "document"
            }
            document_rows = []
            if document_ids:
                cursor.execute(
                    """
                    SELECT id, original_filename
                    FROM documents
                    WHERE workspace_id = %s
                        AND id = ANY(%s::uuid[])
                    """,
                    (workspace_id, list(document_ids)),
                )
                document_rows = cursor.fetchall()

            bill_ids = {
                str(row[key])
                for row in relationship_rows
                for key in ("source_entity_id", "target_entity_id")
                if row[f"{key.split('_')[0]}_entity_type"] == "bill"
            }
            bill_rows = []
            if bill_ids:
                cursor.execute(
                    """
                    SELECT id, supplier, invoice_number
                    FROM bills
                    WHERE workspace_id = %s
                        AND id = ANY(%s::uuid[])
                    """,
                    (workspace_id, list(bill_ids)),
                )
                bill_rows = cursor.fetchall()

    nodes = [_graph_node_from_entity(row) for row in entity_rows]
    nodes.extend(
        HouseholdGraphNode(
            id=str(row["id"]),
            node_type="document",
            display_name=row["original_filename"],
        )
        for row in document_rows
    )
    nodes.extend(
        HouseholdGraphNode(
            id=str(row["id"]),
            node_type="bill",
            display_name=(
                f"{row['supplier']} {row['invoice_number']}"
                if row["invoice_number"]
                else row["supplier"]
            ),
        )
        for row in bill_rows
    )

    return HouseholdGraph(
        nodes=nodes,
        relationships=[_relationship_from_row(row) for row in relationship_rows],
    )


@router.post(
    "/relationships",
    response_model=EntityRelationshipActionResponse,
    status_code=status.HTTP_201_CREATED,
)
def create_relationship(
    request: EntityRelationshipCreate,
) -> EntityRelationshipActionResponse:
    relationship_type = _normalize_kind(request.relationship_type)

    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            source_entity_type = _resolve_relationship_node(
                cursor,
                workspace_id=workspace_id,
                requested_entity_type=request.source_entity_type,
                entity_id=request.source_entity_id,
                side="source",
            )
            target_entity_type = _resolve_relationship_node(
                cursor,
                workspace_id=workspace_id,
                requested_entity_type=request.target_entity_type,
                entity_id=request.target_entity_id,
                side="target",
            )

            if (
                source_entity_type == target_entity_type
                and request.source_entity_id == request.target_entity_id
            ):
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="Source and target must be different graph nodes.",
                )

            cursor.execute(
                """
                INSERT INTO entity_relationships (
                    workspace_id,
                    source_entity_type,
                    source_entity_id,
                    relationship_type,
                    target_entity_type,
                    target_entity_id,
                    provenance_document_id,
                    confidence
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING
                    id,
                    source_entity_type,
                    source_entity_id,
                    relationship_type,
                    target_entity_type,
                    target_entity_id,
                    provenance_document_id,
                    confidence
                """,
                (
                    workspace_id,
                    source_entity_type,
                    request.source_entity_id,
                    relationship_type,
                    target_entity_type,
                    request.target_entity_id,
                    request.provenance_document_id,
                    request.confidence,
                ),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=workspace_id,
                    event_type="relationship.created",
                    entity_type="relationship",
                    entity_id=row["id"],
                    metadata={
                        "source_entity_type": row["source_entity_type"],
                        "source_entity_id": str(row["source_entity_id"]),
                        "relationship_type": row["relationship_type"],
                        "target_entity_type": row["target_entity_type"],
                        "target_entity_id": str(row["target_entity_id"]),
                        "provenance_document_id": (
                            str(row["provenance_document_id"])
                            if row["provenance_document_id"]
                            else None
                        ),
                        "confidence": (
                            float(row["confidence"])
                            if row["confidence"] is not None
                            else None
                        ),
                    },
                )

    if row is None:
        raise RuntimeError("Relationship create did not return a saved relationship.")
    return EntityRelationshipActionResponse(relationship=_relationship_from_row(row))


@router.delete(
    "/relationships/{relationship_id}",
    response_model=EntityRelationshipDeleteResponse,
)
def delete_relationship(relationship_id: str) -> EntityRelationshipDeleteResponse:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                DELETE FROM entity_relationships
                WHERE workspace_id = %s
                    AND id = %s
                RETURNING
                    id,
                    source_entity_type,
                    source_entity_id,
                    relationship_type,
                    target_entity_type,
                    target_entity_id
                """,
                (workspace_id, relationship_id),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=workspace_id,
                    event_type="relationship.deleted",
                    entity_type="relationship",
                    entity_id=row["id"],
                    metadata={
                        "source_entity_type": row["source_entity_type"],
                        "source_entity_id": str(row["source_entity_id"]),
                        "relationship_type": row["relationship_type"],
                        "target_entity_type": row["target_entity_type"],
                        "target_entity_id": str(row["target_entity_id"]),
                    },
                )

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Relationship not found.",
        )
    return EntityRelationshipDeleteResponse(deleted_relationship_id=str(row["id"]))


@router.get("/suggestions", response_model=GraphSuggestionList)
def graph_suggestions() -> GraphSuggestionList:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            suggestions = list_pending_graph_suggestions(cursor, workspace_id=workspace_id)

    return GraphSuggestionList(suggestions=suggestions)


@router.post(
    "/suggestions/{suggestion_id}/accept",
    response_model=GraphSuggestionActionResponse,
)
def accept_suggestion(suggestion_id: str) -> GraphSuggestionActionResponse:
    try:
        with get_connection() as connection:
            workspace_id = get_default_workspace_id(connection)
            with connection.cursor() as cursor:
                suggestion = accept_graph_suggestion(
                    cursor,
                    workspace_id=workspace_id,
                    suggestion_id=suggestion_id,
                )
    except LookupError as error:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=str(error),
        ) from error

    return GraphSuggestionActionResponse(suggestion=suggestion)


@router.post(
    "/suggestions/{suggestion_id}/reject",
    response_model=GraphSuggestionActionResponse,
)
def reject_suggestion(suggestion_id: str) -> GraphSuggestionActionResponse:
    try:
        with get_connection() as connection:
            workspace_id = get_default_workspace_id(connection)
            with connection.cursor() as cursor:
                suggestion = reject_graph_suggestion(
                    cursor,
                    workspace_id=workspace_id,
                    suggestion_id=suggestion_id,
                )
    except LookupError as error:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=str(error),
        ) from error

    return GraphSuggestionActionResponse(suggestion=suggestion)


@router.get("/summary", response_model=HouseholdSummary)
def household_summary() -> HouseholdSummary:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, entity_type, display_name, metadata
                FROM household_entities
                ORDER BY created_at DESC
                LIMIT 12
                """
            )
            entity_rows = cursor.fetchall()

            cursor.execute(
                """
                SELECT id, title, description, status, due_date, related_entity_type, related_entity_id
                FROM tasks
                WHERE status = 'open'
                ORDER BY due_date NULLS LAST, created_at DESC
                LIMIT 8
                """
            )
            task_rows = cursor.fetchall()

            cursor.execute(
                """
                SELECT id, title, remind_at, status, related_entity_type, related_entity_id
                FROM reminders
                WHERE status = 'scheduled'
                ORDER BY remind_at
                LIMIT 8
                """
            )
            reminder_rows = cursor.fetchall()

    return HouseholdSummary(
        entities=[
            _entity_from_row(row)
            for row in entity_rows
        ],
        open_tasks=[_task_from_row(row) for row in task_rows],
        upcoming_reminders=[
            _reminder_from_row(row)
            for row in reminder_rows
        ],
    )


@router.post("/tasks", response_model=TaskActionResponse, status_code=status.HTTP_201_CREATED)
def create_task(request: TaskCreate) -> TaskActionResponse:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                INSERT INTO tasks (
                    workspace_id,
                    title,
                    description,
                    due_date,
                    related_entity_type,
                    related_entity_id
                )
                VALUES (%s, %s, %s, %s, %s, %s)
                RETURNING
                    id,
                    workspace_id,
                    title,
                    description,
                    status,
                    due_date,
                    related_entity_type,
                    related_entity_id
                """,
                (
                    workspace_id,
                    request.title.strip(),
                    request.description,
                    request.due_date,
                    request.related_entity_type,
                    request.related_entity_id,
                ),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=workspace_id,
                    event_type="task.created",
                    entity_type="task",
                    entity_id=row["id"],
                    metadata={
                        "title": row["title"],
                        "related_entity_type": row["related_entity_type"],
                        "related_entity_id": (
                            str(row["related_entity_id"]) if row["related_entity_id"] else None
                        ),
                    },
                )

    if row is None:
        raise RuntimeError("Task create did not return a saved task.")
    return TaskActionResponse(task=_task_from_row(row))


@router.post("/tasks/{task_id}/complete", response_model=TaskActionResponse)
def complete_task(task_id: str) -> TaskActionResponse:
    return _set_task_status(task_id, TaskStatus.done)


@router.post("/tasks/{task_id}/dismiss", response_model=TaskActionResponse)
def dismiss_task(task_id: str) -> TaskActionResponse:
    return _set_task_status(task_id, TaskStatus.dismissed)


@router.post("/tasks/{task_id}/archive", response_model=TaskActionResponse)
def archive_task(task_id: str) -> TaskActionResponse:
    return _set_task_status(task_id, TaskStatus.archived)


def _set_task_status(task_id: str, task_status: TaskStatus) -> TaskActionResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                UPDATE tasks
                SET status = %s, updated_at = now()
                WHERE id = %s
                RETURNING
                    id,
                    workspace_id,
                    title,
                    description,
                    status,
                    due_date,
                    related_entity_type,
                    related_entity_id
                """,
                (task_status.value, task_id),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=row["workspace_id"],
                    event_type=f"task.{task_status.value}",
                    entity_type="task",
                    entity_id=row["id"],
                    metadata={"title": row["title"], "status": row["status"]},
                )

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Task not found.",
        )
    return TaskActionResponse(task=_task_from_row(row))


@router.post(
    "/reminders",
    response_model=ReminderActionResponse,
    status_code=status.HTTP_201_CREATED,
)
def create_reminder(request: ReminderCreate) -> ReminderActionResponse:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                INSERT INTO reminders (
                    workspace_id,
                    title,
                    remind_at,
                    related_entity_type,
                    related_entity_id
                )
                VALUES (%s, %s, %s, %s, %s)
                RETURNING
                    id,
                    workspace_id,
                    title,
                    remind_at,
                    status,
                    related_entity_type,
                    related_entity_id
                """,
                (
                    workspace_id,
                    request.title.strip(),
                    request.remind_at,
                    request.related_entity_type,
                    request.related_entity_id,
                ),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=workspace_id,
                    event_type="reminder.created",
                    entity_type="reminder",
                    entity_id=row["id"],
                    metadata={
                        "title": row["title"],
                        "remind_at": row["remind_at"].isoformat(),
                        "related_entity_type": row["related_entity_type"],
                        "related_entity_id": (
                            str(row["related_entity_id"]) if row["related_entity_id"] else None
                        ),
                    },
                )

    if row is None:
        raise RuntimeError("Reminder create did not return a saved reminder.")
    return ReminderActionResponse(reminder=_reminder_from_row(row))


@router.post("/reminders/{reminder_id}/dismiss", response_model=ReminderActionResponse)
def dismiss_reminder(reminder_id: str) -> ReminderActionResponse:
    return _set_reminder_status(reminder_id, ReminderStatus.dismissed)


@router.post("/reminders/{reminder_id}/archive", response_model=ReminderActionResponse)
def archive_reminder(reminder_id: str) -> ReminderActionResponse:
    return _set_reminder_status(reminder_id, ReminderStatus.archived)


def _set_reminder_status(
    reminder_id: str,
    reminder_status: ReminderStatus,
) -> ReminderActionResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                UPDATE reminders
                SET status = %s, updated_at = now()
                WHERE id = %s
                RETURNING
                    id,
                    workspace_id,
                    title,
                    remind_at,
                    status,
                    related_entity_type,
                    related_entity_id
                """,
                (reminder_status.value, reminder_id),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=row["workspace_id"],
                    event_type=f"reminder.{reminder_status.value}",
                    entity_type="reminder",
                    entity_id=row["id"],
                    metadata={"title": row["title"], "status": row["status"]},
                )

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Reminder not found.",
        )
    return ReminderActionResponse(reminder=_reminder_from_row(row))
