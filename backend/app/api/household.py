from fastapi import APIRouter, HTTPException, status
from psycopg.types.json import Jsonb

from app.db import get_connection, get_default_workspace_id
from app.models.household import (
    EntityRelationship,
    EntityRelationshipActionResponse,
    EntityRelationshipCreate,
    HouseholdEntityActionResponse,
    HouseholdEntityCreate,
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

    if row is None:
        raise RuntimeError("Entity create did not return a saved entity.")
    return HouseholdEntityActionResponse(entity=_entity_from_row(row))


@router.get("/graph", response_model=HouseholdGraph)
def household_graph() -> HouseholdGraph:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, entity_type, display_name, metadata
                FROM household_entities
                ORDER BY created_at
                """
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
                ORDER BY created_at
                """
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
                    WHERE id = ANY(%s::uuid[])
                    """,
                    (list(document_ids),),
                )
                document_rows = cursor.fetchall()

    nodes = [_graph_node_from_entity(row) for row in entity_rows]
    nodes.extend(
        HouseholdGraphNode(
            id=str(row["id"]),
            node_type="document",
            display_name=row["original_filename"],
        )
        for row in document_rows
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
            cursor.execute(
                """
                SELECT id, entity_type
                FROM household_entities
                WHERE workspace_id = %s
                    AND id IN (%s, %s)
                """,
                (workspace_id, request.source_entity_id, request.target_entity_id),
            )
            rows = cursor.fetchall()
            entities = {str(row["id"]): row["entity_type"] for row in rows}

            if request.source_entity_id not in entities or request.target_entity_id not in entities:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="Source and target entities must both exist in the household graph.",
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
                    entities[request.source_entity_id],
                    request.source_entity_id,
                    relationship_type,
                    entities[request.target_entity_id],
                    request.target_entity_id,
                    request.provenance_document_id,
                    request.confidence,
                ),
            )
            row = cursor.fetchone()

    if row is None:
        raise RuntimeError("Relationship create did not return a saved relationship.")
    return EntityRelationshipActionResponse(relationship=_relationship_from_row(row))


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
                RETURNING id, title, description, status, due_date, related_entity_type, related_entity_id
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
                RETURNING id, title, description, status, due_date, related_entity_type, related_entity_id
                """,
                (task_status.value, task_id),
            )
            row = cursor.fetchone()

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
                RETURNING id, title, remind_at, status, related_entity_type, related_entity_id
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
                RETURNING id, title, remind_at, status, related_entity_type, related_entity_id
                """,
                (reminder_status.value, reminder_id),
            )
            row = cursor.fetchone()

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Reminder not found.",
        )
    return ReminderActionResponse(reminder=_reminder_from_row(row))
