from dataclasses import dataclass, field
from typing import Any

from app.api import work as work_api
from app.models.work import ApprovalRequestStatus
from app.services.cabinet import prepare_document_cabinet_plan_work


WORKSPACE_ID = "00000000-0000-0000-0000-000000000001"
DOCUMENT_ID = "20000000-0000-0000-0000-000000000001"
WORK_ORDER_ID = "70000000-0000-0000-0000-000000000001"
APPROVAL_ID = "80000000-0000-0000-0000-000000000001"


@dataclass
class CabinetWorkflowState:
    documents: list[dict[str, Any]] = field(default_factory=list)
    work_orders: list[dict[str, Any]] = field(default_factory=list)
    approvals: list[dict[str, Any]] = field(default_factory=list)
    audit_events: list[dict[str, Any]] = field(default_factory=list)


class FakeCursor:
    def __init__(self, state: CabinetWorkflowState):
        self.state = state
        self.rowcount = 0
        self._next_one: dict[str, Any] | None = None

    def execute(self, sql: str, params: tuple[Any, ...] | None = None) -> None:
        normalized = " ".join(sql.lower().split())
        params = params or ()
        self.rowcount = 0
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
            self.rowcount = 1
            return

        if normalized.startswith("insert into work_orders"):
            (
                workspace_id,
                work_type,
                title,
                description,
                status,
                capability_required,
                subject_entity_type,
                subject_entity_id,
                source_document_id,
                source_bill_id,
                evidence,
            ) = params
            work_order = {
                "workspace_id": workspace_id,
                "id": WORK_ORDER_ID,
                "work_type": work_type,
                "title": title,
                "description": description,
                "status": status,
                "capability_required": capability_required,
                "subject_entity_type": subject_entity_type,
                "subject_entity_id": subject_entity_id,
                "source_document_id": source_document_id,
                "source_bill_id": source_bill_id,
                "evidence": json_value(evidence),
                "result": {},
            }
            self.state.work_orders.append(work_order)
            self.rowcount = 1
            self._next_one = work_order
            return

        if normalized.startswith("insert into approval_requests"):
            workspace_id, work_order_id, requested_approver_role, reason = params
            approval = {
                "workspace_id": workspace_id,
                "id": APPROVAL_ID,
                "work_order_id": work_order_id,
                "status": "pending",
                "requested_approver_role": requested_approver_role,
                "reason": reason,
                "decision_reason": None,
            }
            self.state.approvals.append(approval)
            self.rowcount = 1
            self._next_one = approval
            return

        if normalized.startswith("update work_orders"):
            if len(params) == 4:
                status, result, workspace_id, work_order_id = params
            else:
                result, workspace_id, work_order_id = params
                status = "executed"
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
            self.rowcount = 1
            self._next_one = work_order
            return

        if normalized.startswith("select suggested_cabinet_path"):
            (document_id,) = params
            document = self._document(document_id)
            self._next_one = (
                {"suggested_cabinet_path": document["suggested_cabinet_path"]}
                if document
                else None
            )
            return

        if normalized.startswith("update documents set cabinet_status = 'confirmed'"):
            confirmed_path, document_id = params
            document = self._document(document_id)
            if document is None:
                return
            document["cabinet_status"] = "confirmed"
            document["confirmed_cabinet_path"] = confirmed_path
            self.rowcount = 1
            return

        if normalized.startswith("update documents set cabinet_status = 'needs_review'"):
            (document_id,) = params
            document = self._document(document_id)
            if document is None:
                return
            document["cabinet_status"] = "needs_review"
            self.rowcount = 1
            return

        raise AssertionError(f"Unhandled SQL: {normalized}")

    def fetchone(self) -> dict[str, Any] | None:
        return self._next_one

    def _document(self, document_id: str) -> dict[str, Any] | None:
        return next(
            (document for document in self.state.documents if document["id"] == document_id),
            None,
        )


def json_value(value: Any) -> Any:
    return getattr(value, "obj", value)


def test_cabinet_plan_creates_approval_required_work_order() -> None:
    state = CabinetWorkflowState()
    cursor = FakeCursor(state)

    prepared = prepare_document_cabinet_plan_work(
        cursor,
        workspace_id=WORKSPACE_ID,
        plan={
            "document_id": DOCUMENT_ID,
            "storage_provider": "local_folder",
            "cabinet_status": "suggested",
            "suggested_cabinet_path": "Inbox/Needs-Review/example.pdf",
            "document_classification": "household_document",
            "reasons": ["No household graph placement was available."],
        },
    )

    assert prepared.work_order.status == "approval_requested"
    assert prepared.approval_request is not None
    assert prepared.approval_request.status == "pending"
    assert state.work_orders[0]["evidence"]["requires_approval"] is True
    assert state.approvals[0]["requested_approver_role"] == "owner"
    assert "document.cabinet_plan_suggested" in [
        event["event_type"] for event in state.audit_events
    ]
    assert "authority.evaluated" in [event["event_type"] for event in state.audit_events]


def test_approving_cabinet_plan_confirms_path_without_moving_source() -> None:
    state = CabinetWorkflowState(
        documents=[
            {
                "id": DOCUMENT_ID,
                "storage_path": "storage/documents/original.pdf",
                "cabinet_status": "suggested",
                "suggested_cabinet_path": "Inbox/Needs-Review/example.pdf",
                "confirmed_cabinet_path": None,
            }
        ],
        work_orders=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": WORK_ORDER_ID,
                "work_type": "document.cabinet_plan",
                "title": "Prepare document filing suggestion",
                "description": None,
                "status": "approved",
                "capability_required": "write",
                "subject_entity_type": "document",
                "subject_entity_id": DOCUMENT_ID,
                "source_document_id": DOCUMENT_ID,
                "source_bill_id": None,
                "evidence": {
                    "suggested_cabinet_path": "Inbox/Needs-Review/example.pdf",
                },
                "result": {},
            }
        ],
    )
    cursor = FakeCursor(state)

    updated = work_api._apply_approval_side_effects(
        cursor,
        workspace_id=WORKSPACE_ID,
        work_row=state.work_orders[0],
        decision=ApprovalRequestStatus.approved,
        reason="Approved in Luna Workbench.",
    )

    assert state.documents[0]["cabinet_status"] == "confirmed"
    assert state.documents[0]["confirmed_cabinet_path"] == "Inbox/Needs-Review/example.pdf"
    assert state.documents[0]["storage_path"] == "storage/documents/original.pdf"
    assert updated["status"] == "executed"
    assert updated["result"]["cabinet_status"] == "confirmed"
    assert "document.cabinet_path_confirmed" in [
        event["event_type"] for event in state.audit_events
    ]


def test_rejecting_cabinet_plan_marks_review_without_moving_source() -> None:
    state = CabinetWorkflowState(
        documents=[
            {
                "id": DOCUMENT_ID,
                "storage_path": "storage/documents/original.pdf",
                "cabinet_status": "suggested",
                "suggested_cabinet_path": "Inbox/Needs-Review/example.pdf",
                "confirmed_cabinet_path": None,
            }
        ],
        work_orders=[
            {
                "workspace_id": WORKSPACE_ID,
                "id": WORK_ORDER_ID,
                "work_type": "document.cabinet_plan",
                "title": "Prepare document filing suggestion",
                "description": None,
                "status": "rejected",
                "capability_required": "write",
                "subject_entity_type": "document",
                "subject_entity_id": DOCUMENT_ID,
                "source_document_id": DOCUMENT_ID,
                "source_bill_id": None,
                "evidence": {
                    "suggested_cabinet_path": "Inbox/Needs-Review/example.pdf",
                },
                "result": {},
            }
        ],
    )
    cursor = FakeCursor(state)

    work_api._apply_approval_side_effects(
        cursor,
        workspace_id=WORKSPACE_ID,
        work_row=state.work_orders[0],
        decision=ApprovalRequestStatus.rejected,
        reason="Wrong folder.",
    )

    assert state.documents[0]["cabinet_status"] == "needs_review"
    assert state.documents[0]["confirmed_cabinet_path"] is None
    assert state.documents[0]["storage_path"] == "storage/documents/original.pdf"
    assert "document.cabinet_plan_not_approved" in [
        event["event_type"] for event in state.audit_events
    ]
