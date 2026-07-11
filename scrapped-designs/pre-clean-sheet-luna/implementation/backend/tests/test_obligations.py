from dataclasses import dataclass, field
from datetime import date
from typing import Any

from app.services.obligations import (
    backfill_confirmed_bill_obligations,
    refresh_overdue_obligations,
    sync_bill_obligation,
)


WORKSPACE_ID = "00000000-0000-0000-0000-000000000001"
BILL_ID = "30000000-0000-0000-0000-000000000001"
OBLIGATION_ID = "90000000-0000-0000-0000-000000000001"
REMINDER_ID = "91000000-0000-0000-0000-000000000001"


@dataclass
class ObligationState:
    bills: list[dict[str, Any]] = field(default_factory=list)
    obligations: list[dict[str, Any]] = field(default_factory=list)
    reminders: list[dict[str, Any]] = field(default_factory=list)
    audit_events: list[dict[str, Any]] = field(default_factory=list)


class FakeCursor:
    def __init__(self, state: ObligationState):
        self.state = state
        self._next_one: dict[str, Any] | None = None
        self._next_all: list[dict[str, Any]] = []

    def execute(self, sql: str, params: tuple[Any, ...] | None = None) -> None:
        normalized = " ".join(sql.lower().split())
        params = params or ()
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
                    "metadata": json_value(metadata),
                }
            )
            return

        if normalized.startswith("select b.id"):
            (workspace_id,) = params
            bill_rows: list[dict[str, Any]] = []
            for bill in self.state.bills:
                if bill["workspace_id"] != workspace_id or bill["review_status"] != "confirmed":
                    continue
                if self._obligation_by_bill(workspace_id, bill["id"]) is not None:
                    continue
                bill_rows.append(bill)
            self._next_all = bill_rows
            return

        if normalized.startswith("select id, status from obligations"):
            workspace_id, bill_id = params
            self._next_one = self._obligation_by_bill(workspace_id, bill_id)
            return

        if normalized.startswith("insert into obligations"):
            (
                workspace_id,
                bill_id,
                title,
                supplier,
                amount,
                currency,
                due_date,
                status,
                evidence,
            ) = params
            obligation = self._obligation_by_bill(workspace_id, bill_id)
            inserted = obligation is None
            if obligation is None:
                obligation = {
                    "workspace_id": workspace_id,
                    "id": OBLIGATION_ID,
                    "source_bill_id": bill_id,
                    "evidence": {},
                }
                self.state.obligations.append(obligation)
            obligation.update(
                {
                    "title": title,
                    "supplier": supplier,
                    "amount": amount,
                    "currency": currency,
                    "due_date": due_date,
                    "status": status,
                    "evidence": {
                        **obligation.get("evidence", {}),
                        **json_value(evidence),
                    },
                    "inserted": inserted,
                }
            )
            self._next_one = obligation
            return

        if normalized.startswith("select id from reminders"):
            workspace_id, obligation_id = params
            self._next_one = next(
                (
                    reminder
                    for reminder in self.state.reminders
                    if reminder["workspace_id"] == workspace_id
                    and reminder["related_entity_type"] == "obligation"
                    and reminder["related_entity_id"] == obligation_id
                    and reminder["status"] == "scheduled"
                ),
                None,
            )
            return

        if normalized.startswith("insert into reminders"):
            workspace_id, title, remind_at, obligation_id = params
            reminder = {
                "workspace_id": workspace_id,
                "id": REMINDER_ID,
                "title": title,
                "remind_at": remind_at,
                "related_entity_type": "obligation",
                "related_entity_id": obligation_id,
                "status": "scheduled",
            }
            self.state.reminders.append(reminder)
            self._next_one = reminder
            return

        if normalized.startswith("update reminders"):
            title, remind_at, reminder_id = params
            for reminder in self.state.reminders:
                if reminder["id"] == reminder_id:
                    reminder["title"] = title
                    reminder["remind_at"] = remind_at
                    return
            return

        if normalized.startswith("update obligations set status = 'overdue'"):
            (workspace_id,) = params
            changed: list[dict[str, Any]] = []
            today = date.today()
            for obligation in self.state.obligations:
                if (
                    obligation["workspace_id"] == workspace_id
                    and obligation["status"] in {"upcoming", "due_soon"}
                    and obligation["due_date"] < today
                ):
                    obligation["status"] = "overdue"
                    changed.append(obligation)
            self._next_all = changed
            return

        raise AssertionError(f"Unhandled SQL: {normalized}")

    def fetchone(self) -> dict[str, Any] | None:
        return self._next_one

    def fetchall(self) -> list[dict[str, Any]]:
        return self._next_all

    def _obligation_by_bill(
        self,
        workspace_id: str,
        bill_id: str,
    ) -> dict[str, Any] | None:
        return next(
            (
                obligation
                for obligation in self.state.obligations
                if obligation["workspace_id"] == workspace_id
                and obligation["source_bill_id"] == bill_id
            ),
            None,
        )


def json_value(value: Any) -> Any:
    return getattr(value, "obj", value)


def bill_row(
    *,
    status: str = "unpaid",
    review_status: str = "confirmed",
    due_date: date | None = date(2026, 7, 20),
) -> dict[str, Any]:
    return {
        "id": BILL_ID,
        "document_id": "20000000-0000-0000-0000-000000000001",
        "supplier": "AGL",
        "amount": 123.45,
        "currency": "AUD",
        "due_date": due_date,
        "status": status,
        "review_status": review_status,
    }


def test_confirmed_bill_creates_obligation_and_due_date_reminder() -> None:
    state = ObligationState()
    cursor = FakeCursor(state)

    obligation = sync_bill_obligation(
        cursor,
        workspace_id=WORKSPACE_ID,
        bill=bill_row(),
    )

    assert obligation is not None
    assert obligation["status"] == "upcoming"
    assert state.obligations[0]["source_bill_id"] == BILL_ID
    assert state.reminders[0]["related_entity_type"] == "obligation"
    assert state.reminders[0]["related_entity_id"] == OBLIGATION_ID
    assert "obligation.created" in [event["event_type"] for event in state.audit_events]
    assert "reminder.created" in [event["event_type"] for event in state.audit_events]


def test_bill_status_change_updates_obligation_and_audits_transition() -> None:
    state = ObligationState(
        obligations=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": OBLIGATION_ID,
                "source_bill_id": BILL_ID,
                "title": "AGL 123.45 AUD",
                "supplier": "AGL",
                "amount": 123.45,
                "currency": "AUD",
                "due_date": date(2026, 7, 20),
                "status": "upcoming",
                "evidence": {},
            }
        ]
    )
    cursor = FakeCursor(state)

    sync_bill_obligation(
        cursor,
        workspace_id=WORKSPACE_ID,
        bill=bill_row(status="paid"),
    )

    assert state.obligations[0]["status"] == "paid"
    assert "obligation.updated" in [event["event_type"] for event in state.audit_events]
    assert "obligation.status_changed" in [
        event["event_type"] for event in state.audit_events
    ]


def test_refresh_overdue_obligations_audits_status_change() -> None:
    state = ObligationState(
        obligations=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": OBLIGATION_ID,
                "source_bill_id": BILL_ID,
                "title": "AGL 123.45 AUD",
                "supplier": "AGL",
                "amount": 123.45,
                "currency": "AUD",
                "due_date": date(2020, 1, 1),
                "status": "upcoming",
                "evidence": {},
            }
        ]
    )
    cursor = FakeCursor(state)

    changed = refresh_overdue_obligations(cursor, workspace_id=WORKSPACE_ID)

    assert len(changed) == 1
    assert state.obligations[0]["status"] == "overdue"
    assert "obligation.status_changed" in [
        event["event_type"] for event in state.audit_events
    ]


def test_backfill_confirmed_bills_without_obligations() -> None:
    state = ObligationState(
        bills=[
            {
                **bill_row(),
                "workspace_id": WORKSPACE_ID,
            },
            {
                **bill_row(),
                "id": "30000000-0000-0000-0000-000000000002",
                "workspace_id": WORKSPACE_ID,
                "review_status": "needs_review",
            },
        ]
    )
    cursor = FakeCursor(state)

    backfilled = backfill_confirmed_bill_obligations(
        cursor,
        workspace_id=WORKSPACE_ID,
    )

    assert len(backfilled) == 1
    assert state.obligations[0]["source_bill_id"] == BILL_ID
    assert state.reminders[0]["related_entity_type"] == "obligation"
    assert "obligation.backfill_completed" in [
        event["event_type"] for event in state.audit_events
    ]
