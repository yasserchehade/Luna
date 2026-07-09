from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any

import pytest

from app.api import work as work_api
from app.models.work import ApprovalDecisionRequest


WORKSPACE_ID = "00000000-0000-0000-0000-000000000001"
WORK_ORDER_ID = "70000000-0000-0000-0000-000000000001"
APPROVAL_ID = "80000000-0000-0000-0000-000000000001"


@dataclass
class ApprovalState:
    work_orders: list[dict[str, Any]] = field(default_factory=list)
    approvals: list[dict[str, Any]] = field(default_factory=list)
    audit_events: list[dict[str, Any]] = field(default_factory=list)


class FakeConnection:
    def __init__(self, state: ApprovalState):
        self.state = state

    def __enter__(self) -> "FakeConnection":
        return self

    def __exit__(self, *_exc: object) -> None:
        return None

    def cursor(self) -> "FakeCursor":
        return FakeCursor(self.state)


class FakeCursor:
    def __init__(self, state: ApprovalState):
        self.state = state
        self._next_one: dict[str, Any] | None = None

    def __enter__(self) -> "FakeCursor":
        return self

    def __exit__(self, *_exc: object) -> None:
        return None

    def execute(self, sql: str, params: tuple[Any, ...] | None = None) -> None:
        normalized = " ".join(sql.lower().split())
        params = params or ()
        self._next_one = None

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

        if normalized.startswith("select work_order_id from approval_requests"):
            workspace_id, approval_id = params
            self._next_one = next(
                (
                    approval
                    for approval in self.state.approvals
                    if approval["workspace_id"] == workspace_id
                    and approval["id"] == approval_id
                ),
                None,
            )
            return

        if normalized.startswith("update approval_requests"):
            status, reason, workspace_id, work_order_id = params
            approval = next(
                (
                    approval
                    for approval in self.state.approvals
                    if approval["workspace_id"] == workspace_id
                    and approval["work_order_id"] == work_order_id
                    and approval["status"] == "pending"
                ),
                None,
            )
            if approval is None:
                return
            approval["status"] = status
            approval["decision_reason"] = reason
            self._next_one = approval
            return

        if normalized.startswith("update work_orders"):
            status, result, workspace_id, work_order_id = params
            work_order = next(
                (
                    order
                    for order in self.state.work_orders
                    if order["workspace_id"] == workspace_id
                    and order["id"] == work_order_id
                ),
                None,
            )
            if work_order is None:
                return
            work_order["status"] = status
            work_order["result"] = {
                **work_order["result"],
                **json_value(result),
            }
            self._next_one = work_order
            return

        if "from work_orders" in normalized:
            workspace_id, work_order_id = params
            self._next_one = next(
                (
                    order
                    for order in self.state.work_orders
                    if order["workspace_id"] == workspace_id
                    and order["id"] == work_order_id
                ),
                None,
            )
            return

        raise AssertionError(f"Unhandled SQL: {normalized}")

    def fetchone(self) -> dict[str, Any] | None:
        return self._next_one


def json_value(value: Any) -> Any:
    return getattr(value, "obj", value)


@contextmanager
def fake_connection(state: ApprovalState) -> Iterator[FakeConnection]:
    yield FakeConnection(state)


def build_state() -> ApprovalState:
    return ApprovalState(
        work_orders=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": WORK_ORDER_ID,
                "work_type": "bill.prepare_for_approval",
                "title": "Prepare AGL bill for approval",
                "description": "Luna prepared a bill from a household record.",
                "status": "approval_requested",
                "capability_required": "write",
                "subject_entity_type": "bill",
                "subject_entity_id": None,
                "source_document_id": "20000000-0000-0000-0000-000000000001",
                "source_bill_id": "30000000-0000-0000-0000-000000000001",
                "evidence": {"confidence": 0.91, "source": "bill_upload"},
                "result": {},
            }
        ],
        approvals=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": APPROVAL_ID,
                "work_order_id": WORK_ORDER_ID,
                "status": "pending",
                "requested_approver_role": "owner",
                "reason": "Luna needs approval before filing this obligation.",
                "decision_reason": None,
            }
        ],
    )


@pytest.mark.parametrize(
    ("action", "expected_status", "expected_event"),
    [
        ("approve_request", "approved", "approval_request.approved"),
        ("reject_request", "rejected", "approval_request.rejected"),
        ("dismiss_request", "dismissed", "approval_request.dismissed"),
    ],
)
def test_approval_decision_updates_work_order_and_audit_trail(
    monkeypatch: pytest.MonkeyPatch,
    action: str,
    expected_status: str,
    expected_event: str,
) -> None:
    state = build_state()
    monkeypatch.setattr(work_api, "get_connection", lambda: fake_connection(state))
    monkeypatch.setattr(work_api, "get_default_workspace_id", lambda _connection: WORKSPACE_ID)

    response = getattr(work_api, action)(
        APPROVAL_ID,
        ApprovalDecisionRequest(reason="Reviewed in Luna Workbench."),
    )

    assert response.approval_request.status == expected_status
    assert response.work_order.status == expected_status
    assert state.approvals[0]["decision_reason"] == "Reviewed in Luna Workbench."
    assert state.work_orders[0]["status"] == expected_status
    if expected_status in {"rejected", "dismissed"}:
        assert state.work_orders[0]["result"]["reason"] == "Reviewed in Luna Workbench."
    else:
        assert state.work_orders[0]["result"] == {}
    assert expected_event in [event["event_type"] for event in state.audit_events]
    assert f"work_order.{expected_status}" in [
        event["event_type"] for event in state.audit_events
    ]
