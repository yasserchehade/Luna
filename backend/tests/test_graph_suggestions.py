from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any

from app.api import household


ACTIVE_WORKSPACE_ID = "00000000-0000-0000-0000-000000000001"
PROPERTY_ID = "10000000-0000-0000-0000-000000000001"
SUPPLIER_ID = "10000000-0000-0000-0000-000000000002"
DOCUMENT_ID = "20000000-0000-0000-0000-000000000001"
BILL_ID = "30000000-0000-0000-0000-000000000001"
SUGGESTION_ID = "40000000-0000-0000-0000-000000000001"
RELATIONSHIP_ID = "50000000-0000-0000-0000-000000000001"


@dataclass
class SuggestionState:
    suggestions: list[dict[str, Any]] = field(default_factory=list)
    relationships: list[dict[str, Any]] = field(default_factory=list)
    audit_events: list[dict[str, Any]] = field(default_factory=list)


class FakeConnection:
    def __init__(self, state: SuggestionState):
        self.state = state

    def cursor(self) -> "FakeCursor":
        return FakeCursor(self.state)


class FakeCursor:
    def __init__(self, state: SuggestionState):
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
        self._next_one = None
        self._next_all = []

        if normalized.startswith("create table") or normalized.startswith("create index"):
            return

        if "insert into audit_events" in normalized:
            workspace_id, event_type, entity_type, entity_id, metadata = params
            self.state.audit_events.append(
                {
                    "workspace_id": workspace_id,
                    "event_type": event_type,
                    "entity_type": entity_type,
                    "entity_id": entity_id,
                    "metadata": json_value(metadata),
                }
            )
            return

        if "select id, entity_type, display_name, metadata from household_entities" in normalized:
            self._next_all = [
                {
                    "id": PROPERTY_ID,
                    "entity_type": "property",
                    "display_name": "12 Smith Street",
                    "metadata": {"address": "12 Smith Street"},
                },
                {
                    "id": SUPPLIER_ID,
                    "entity_type": "supplier",
                    "display_name": "AGL",
                    "metadata": {},
                },
            ]
            return

        if normalized.startswith("select source_entity_type"):
            self._next_all = self.state.relationships
            return

        if "from documents d left join document_texts" in normalized:
            self._next_all = [
                {
                    "id": DOCUMENT_ID,
                    "original_filename": "agl-electricity.pdf",
                    "suggested_cabinet_path": None,
                    "confirmed_cabinet_path": None,
                    "text_content": "Electricity bill for 12 Smith Street",
                }
            ]
            return

        if "from bills where workspace_id" in normalized:
            self._next_all = [
                {
                    "id": BILL_ID,
                    "document_id": DOCUMENT_ID,
                    "supplier_entity_id": SUPPLIER_ID,
                    "supplier": "AGL",
                    "invoice_number": "INV-123",
                    "category": "electricity",
                    "classification": "property",
                    "extraction_confidence": 0.91,
                }
            ]
            return

        if normalized.startswith("insert into graph_suggestions"):
            (
                workspace_id,
                fingerprint,
                action_type,
                action_payload,
                confidence,
                reasoning,
                affected_entities,
                source_document_id,
                source_bill_id,
            ) = params
            if not any(
                suggestion["workspace_id"] == workspace_id
                and suggestion["fingerprint"] == fingerprint
                for suggestion in self.state.suggestions
            ):
                index = len(self.state.suggestions) + 1
                self.state.suggestions.append(
                    {
                        "id": SUGGESTION_ID[:-1] + str(index),
                        "workspace_id": workspace_id,
                        "fingerprint": fingerprint,
                        "status": "pending",
                        "action_type": action_type,
                        "action_payload": json_value(action_payload),
                        "confidence": confidence,
                        "reasoning": reasoning,
                        "affected_entities": json_value(affected_entities),
                        "source_document_id": source_document_id,
                        "source_bill_id": source_bill_id,
                    }
                )
            return

        if "from graph_suggestions" in normalized and "status = 'pending'" in normalized:
            if "and id = %s" in normalized:
                suggestion = next(
                    (
                        suggestion
                        for suggestion in self.state.suggestions
                        if suggestion["workspace_id"] == params[0]
                        and suggestion["id"] == params[1]
                        and suggestion["status"] == "pending"
                    ),
                    None,
                )
                self._next_one = suggestion
            else:
                self._next_all = [
                    suggestion
                    for suggestion in self.state.suggestions
                    if suggestion["workspace_id"] == params[0]
                    and suggestion["status"] == "pending"
                ]
            return

        if normalized.startswith("select id from entity_relationships"):
            self._next_one = next(
                (
                    relationship
                    for relationship in self.state.relationships
                    if relationship["source_entity_type"] == params[1]
                    and relationship["source_entity_id"] == params[2]
                    and relationship["relationship_type"] == params[3]
                    and relationship["target_entity_type"] == params[4]
                    and relationship["target_entity_id"] == params[5]
                ),
                None,
            )
            return

        if normalized.startswith("insert into entity_relationships"):
            relationship = {
                "id": RELATIONSHIP_ID,
                "source_entity_type": params[1],
                "source_entity_id": params[2],
                "relationship_type": params[3],
                "target_entity_type": params[4],
                "target_entity_id": params[5],
                "provenance_document_id": params[6],
                "confidence": params[7],
            }
            self.state.relationships.append(relationship)
            self._next_one = relationship
            return

        if normalized.startswith("select id from household_entities"):
            self._next_one = None
            return

        if normalized.startswith("insert into household_entities"):
            self._next_one = {
                "id": "60000000-0000-0000-0000-000000000001",
                "entity_type": params[1],
                "display_name": params[2],
                "metadata": json_value(params[3]),
            }
            return

        if normalized.startswith("update graph_suggestions"):
            status = "accepted" if "status = 'accepted'" in normalized else "rejected"
            suggestion = next(
                suggestion
                for suggestion in self.state.suggestions
                if suggestion["workspace_id"] == params[0] and suggestion["id"] == params[1]
            )
            suggestion["status"] = status
            self._next_one = suggestion
            return

        raise AssertionError(f"Unexpected SQL: {sql}")

    def fetchone(self) -> dict[str, Any] | None:
        return self._next_one

    def fetchall(self) -> list[dict[str, Any]]:
        return self._next_all


def install_fake_db(monkeypatch: Any, state: SuggestionState) -> None:
    @contextmanager
    def fake_get_connection() -> Iterator[FakeConnection]:
        yield FakeConnection(state)

    monkeypatch.setattr(household, "get_connection", fake_get_connection)
    monkeypatch.setattr(
        household,
        "get_default_workspace_id",
        lambda _connection: ACTIVE_WORKSPACE_ID,
    )


def json_value(value: Any) -> Any:
    return getattr(value, "obj", value)


def test_graph_suggestions_are_generated_and_rejections_are_not_recreated(monkeypatch: Any) -> None:
    state = SuggestionState()
    install_fake_db(monkeypatch, state)

    initial = household.graph_suggestions()

    assert len(initial.suggestions) >= 2
    assert {suggestion.suggested_action for suggestion in initial.suggestions} >= {
        "attach_document",
        "connect_entities",
    }

    rejected = household.reject_suggestion(initial.suggestions[0].id)
    after_rejection = household.graph_suggestions()

    assert rejected.suggestion.status == "rejected"
    assert rejected.suggestion.id not in {suggestion.id for suggestion in after_rejection.suggestions}
    assert [event["event_type"] for event in state.audit_events] == ["graph_suggestion.rejected"]


def test_accepting_graph_suggestion_updates_graph_and_records_audit(monkeypatch: Any) -> None:
    state = SuggestionState()
    install_fake_db(monkeypatch, state)
    suggestions = household.graph_suggestions().suggestions
    suggestion = next(
        suggestion
        for suggestion in suggestions
        if suggestion.suggested_action == "connect_entities"
    )

    accepted = household.accept_suggestion(suggestion.id)

    assert accepted.suggestion.status == "accepted"
    assert state.relationships == [
        {
            "id": RELATIONSHIP_ID,
            "source_entity_type": "bill",
            "source_entity_id": BILL_ID,
            "relationship_type": "issued_by",
            "target_entity_type": "supplier",
            "target_entity_id": SUPPLIER_ID,
            "provenance_document_id": DOCUMENT_ID,
            "confidence": 0.91,
        }
    ]
    assert [event["event_type"] for event in state.audit_events] == [
        "relationship.created",
        "graph_suggestion.accepted",
    ]
