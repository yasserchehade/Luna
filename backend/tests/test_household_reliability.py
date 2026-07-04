from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from datetime import date, datetime
from typing import Any

from app.api import household
from app.models.household import EntityRelationshipCreate


ACTIVE_WORKSPACE_ID = "00000000-0000-0000-0000-000000000001"
OTHER_WORKSPACE_ID = "00000000-0000-0000-0000-000000000002"
TASK_ID = "10000000-0000-0000-0000-000000000001"
REMINDER_ID = "20000000-0000-0000-0000-000000000001"
SOURCE_ENTITY_ID = "30000000-0000-0000-0000-000000000001"
TARGET_ENTITY_ID = "30000000-0000-0000-0000-000000000002"
OTHER_ENTITY_ID = "30000000-0000-0000-0000-000000000003"
DOCUMENT_ID = "40000000-0000-0000-0000-000000000001"
OTHER_DOCUMENT_ID = "40000000-0000-0000-0000-000000000002"
RELATIONSHIP_ID = "50000000-0000-0000-0000-000000000001"


@dataclass
class FakeDbState:
    audit_events: list[dict[str, Any]] = field(default_factory=list)
    executed: list[tuple[str, tuple[Any, ...]]] = field(default_factory=list)


class FakeConnection:
    def __init__(self, state: FakeDbState):
        self.state = state

    def cursor(self) -> "FakeCursor":
        return FakeCursor(self.state)


class FakeCursor:
    def __init__(self, state: FakeDbState):
        self.state = state
        self._next_one: dict[str, Any] | None = None
        self._next_all: list[dict[str, Any]] = []

    def __enter__(self) -> "FakeCursor":
        return self

    def __exit__(self, *_exc: object) -> None:
        return None

    def execute(self, sql: str, params: tuple[Any, ...] | None = None) -> None:
        normalized = " ".join(sql.lower().split())
        params = params or ()
        self.state.executed.append((normalized, params))
        self._next_one = None
        self._next_all = []

        if "insert into audit_events" in normalized:
            workspace_id, event_type, entity_type, entity_id, metadata = params
            self.state.audit_events.append(
                {
                    "workspace_id": workspace_id,
                    "event_type": event_type,
                    "entity_type": entity_type,
                    "entity_id": entity_id,
                    "metadata": metadata,
                }
            )
            return

        if normalized.startswith("update tasks"):
            assert "returning id, workspace_id," in normalized
            self._next_one = {
                "id": TASK_ID,
                "workspace_id": ACTIVE_WORKSPACE_ID,
                "title": "Pay water bill",
                "description": None,
                "status": params[0],
                "due_date": date(2026, 7, 15),
                "related_entity_type": "property",
                "related_entity_id": SOURCE_ENTITY_ID,
            }
            return

        if normalized.startswith("update reminders"):
            assert "returning id, workspace_id," in normalized
            self._next_one = {
                "id": REMINDER_ID,
                "workspace_id": ACTIVE_WORKSPACE_ID,
                "title": "Water bill due",
                "remind_at": datetime(2026, 7, 14, 9, 0, 0),
                "status": params[0],
                "related_entity_type": "property",
                "related_entity_id": SOURCE_ENTITY_ID,
            }
            return

        if "from household_entities" in normalized and "id in" in normalized:
            assert "where workspace_id = %s" in normalized
            assert params[0] == ACTIVE_WORKSPACE_ID
            self._next_all = [
                {"id": SOURCE_ENTITY_ID, "entity_type": "property"},
                {"id": TARGET_ENTITY_ID, "entity_type": "supplier"},
            ]
            return

        if normalized.startswith("insert into entity_relationships"):
            self._next_one = relationship_row()
            return

        if normalized.startswith("delete from entity_relationships"):
            assert "where workspace_id = %s and id = %s" in normalized
            assert params == (ACTIVE_WORKSPACE_ID, RELATIONSHIP_ID)
            self._next_one = relationship_row()
            return

        if "from household_entities" in normalized:
            assert "where workspace_id = %s" in normalized
            assert params == (ACTIVE_WORKSPACE_ID,)
            self._next_all = [
                {
                    "id": SOURCE_ENTITY_ID,
                    "entity_type": "property",
                    "display_name": "Home",
                    "metadata": {},
                    "workspace_id": ACTIVE_WORKSPACE_ID,
                },
                {
                    "id": TARGET_ENTITY_ID,
                    "entity_type": "supplier",
                    "display_name": "Water Supplier",
                    "metadata": {},
                    "workspace_id": ACTIVE_WORKSPACE_ID,
                },
            ]
            return

        if "from entity_relationships" in normalized:
            assert "where workspace_id = %s" in normalized
            assert params == (ACTIVE_WORKSPACE_ID,)
            self._next_all = [
                {
                    **relationship_row(),
                    "source_entity_type": "document",
                    "source_entity_id": DOCUMENT_ID,
                    "target_entity_type": "property",
                    "target_entity_id": SOURCE_ENTITY_ID,
                }
            ]
            return

        if "from documents" in normalized:
            assert "where workspace_id = %s" in normalized
            assert params[0] == ACTIVE_WORKSPACE_ID
            assert DOCUMENT_ID in params[1]
            self._next_all = [{"id": DOCUMENT_ID, "original_filename": "water-bill.pdf"}]
            return

        raise AssertionError(f"Unexpected SQL: {sql}")

    def fetchone(self) -> dict[str, Any] | None:
        return self._next_one

    def fetchall(self) -> list[dict[str, Any]]:
        return self._next_all


def relationship_row() -> dict[str, Any]:
    return {
        "id": RELATIONSHIP_ID,
        "source_entity_type": "property",
        "source_entity_id": SOURCE_ENTITY_ID,
        "relationship_type": "uses_supplier",
        "target_entity_type": "supplier",
        "target_entity_id": TARGET_ENTITY_ID,
        "provenance_document_id": None,
        "confidence": 0.9,
    }


def install_fake_db(monkeypatch: Any, state: FakeDbState) -> None:
    @contextmanager
    def fake_get_connection() -> Iterator[FakeConnection]:
        yield FakeConnection(state)

    monkeypatch.setattr(household, "get_connection", fake_get_connection)
    monkeypatch.setattr(
        household,
        "get_default_workspace_id",
        lambda _connection: ACTIVE_WORKSPACE_ID,
    )


def test_completing_task_records_audit_event(monkeypatch: Any) -> None:
    state = FakeDbState()
    install_fake_db(monkeypatch, state)

    response = household.complete_task(TASK_ID)

    assert response.task.status == "done"
    assert state.audit_events == [
        {
            "workspace_id": ACTIVE_WORKSPACE_ID,
            "event_type": "task.done",
            "entity_type": "task",
            "entity_id": TASK_ID,
            "metadata": state.audit_events[0]["metadata"],
        }
    ]


def test_dismissing_reminder_records_audit_event(monkeypatch: Any) -> None:
    state = FakeDbState()
    install_fake_db(monkeypatch, state)

    response = household.dismiss_reminder(REMINDER_ID)

    assert response.reminder.status == "dismissed"
    assert state.audit_events == [
        {
            "workspace_id": ACTIVE_WORKSPACE_ID,
            "event_type": "reminder.dismissed",
            "entity_type": "reminder",
            "entity_id": REMINDER_ID,
            "metadata": state.audit_events[0]["metadata"],
        }
    ]


def test_household_graph_only_returns_active_workspace_records(monkeypatch: Any) -> None:
    state = FakeDbState()
    install_fake_db(monkeypatch, state)

    graph = household.household_graph()

    node_ids = {node.id for node in graph.nodes}
    relationship_ids = {relationship.id for relationship in graph.relationships}

    assert SOURCE_ENTITY_ID in node_ids
    assert TARGET_ENTITY_ID in node_ids
    assert DOCUMENT_ID in node_ids
    assert OTHER_ENTITY_ID not in node_ids
    assert OTHER_DOCUMENT_ID not in node_ids
    assert relationship_ids == {RELATIONSHIP_ID}


def test_creating_and_deleting_relationship_records_audit_events(monkeypatch: Any) -> None:
    state = FakeDbState()
    install_fake_db(monkeypatch, state)

    created = household.create_relationship(
        EntityRelationshipCreate(
            source_entity_id=SOURCE_ENTITY_ID,
            relationship_type="uses supplier",
            target_entity_id=TARGET_ENTITY_ID,
            confidence=0.9,
        )
    )
    deleted = household.delete_relationship(RELATIONSHIP_ID)

    assert created.relationship.id == RELATIONSHIP_ID
    assert deleted.deleted_relationship_id == RELATIONSHIP_ID
    assert [event["event_type"] for event in state.audit_events] == [
        "relationship.created",
        "relationship.deleted",
    ]
    assert all(event["workspace_id"] == ACTIVE_WORKSPACE_ID for event in state.audit_events)
