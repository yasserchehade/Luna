from collections.abc import Mapping
from typing import Any

from psycopg import Cursor
from psycopg.types.json import Jsonb

from app.models.work import (
    ApprovalRequest,
    ApprovalRequestStatus,
    AuthorityDecision,
    Capability,
    HouseholdRole,
    WorkOrder,
    WorkOrderCreate,
    WorkOrderStatus,
    WorkOrderWithApproval,
)
from app.services.audit import record_audit_event


def create_work_order(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    request: WorkOrderCreate,
) -> WorkOrder:
    cursor.execute(
        """
        INSERT INTO work_orders (
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
            evidence
        )
        VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
        RETURNING
            id,
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
            result
        """,
        (
            workspace_id,
            request.work_type,
            request.title.strip(),
            request.description,
            request.status.value,
            request.capability_required.value,
            request.subject_entity_type,
            request.subject_entity_id,
            request.source_document_id,
            request.source_bill_id,
            Jsonb(request.evidence),
        ),
    )
    row = cursor.fetchone()
    if row is None:
        raise RuntimeError("Work order create did not return a saved row.")
    work_order = _work_order_from_row(row)
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="work_order.created",
        entity_type="work_order",
        entity_id=work_order.id,
        metadata={
            "work_type": work_order.work_type,
            "status": work_order.status.value,
            "capability_required": work_order.capability_required.value,
            "subject_entity_type": work_order.subject_entity_type,
            "subject_entity_id": work_order.subject_entity_id,
            "source_document_id": work_order.source_document_id,
            "source_bill_id": work_order.source_bill_id,
        },
    )
    return work_order


def prepare_work(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    request: WorkOrderCreate,
    approval_reason: str | None = None,
) -> WorkOrderWithApproval:
    work_order = create_work_order(
        cursor,
        workspace_id=workspace_id,
        request=request,
    )
    decision = evaluate_authority(
        cursor,
        workspace_id=workspace_id,
        work_type=request.work_type,
        capability=request.capability_required,
    )
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="authority.evaluated",
        entity_type="work_order",
        entity_id=work_order.id,
        metadata={
            "work_type": request.work_type,
            "capability": request.capability_required.value,
            "decision": decision["decision"].value,
            "approver_role": (
                decision["approver_role"].value
                if isinstance(decision["approver_role"], HouseholdRole)
                else None
            ),
            "policy_id": decision["policy_id"],
        },
    )

    if decision["decision"] == AuthorityDecision.allowed:
        return WorkOrderWithApproval(work_order=work_order)
    if decision["decision"] == AuthorityDecision.approval_required:
        approval = request_approval(
            cursor,
            workspace_id=workspace_id,
            work_order_id=work_order.id,
            reason=approval_reason
            or "Luna needs approval before continuing this work.",
            requested_approver_role=decision["approver_role"],
        )
        return WorkOrderWithApproval(
            work_order=_set_work_order_status(
                cursor,
                workspace_id=workspace_id,
                work_order_id=work_order.id,
                status=WorkOrderStatus.approval_requested,
            ),
            approval_request=approval,
        )
    if decision["decision"] == AuthorityDecision.escalate:
        return WorkOrderWithApproval(
            work_order=_set_work_order_status(
                cursor,
                workspace_id=workspace_id,
                work_order_id=work_order.id,
                status=WorkOrderStatus.escalated,
                result={"reason": "Authority policy requires escalation."},
            )
        )
    return WorkOrderWithApproval(
        work_order=_set_work_order_status(
            cursor,
            workspace_id=workspace_id,
            work_order_id=work_order.id,
            status=WorkOrderStatus.rejected,
            result={"reason": "Authority policy blocks this work."},
        )
    )


def evaluate_authority(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_type: str,
    capability: Capability,
) -> dict[str, object]:
    cursor.execute(
        """
        SELECT id, decision, approver_role, spending_limit, escalation_rule
        FROM authority_policies
        WHERE workspace_id = %s
            AND work_type = %s
            AND capability = %s
            AND enabled = true
        LIMIT 1
        """,
        (workspace_id, work_type, capability.value),
    )
    row = cursor.fetchone()
    if row is not None:
        return {
            "policy_id": str(row["id"]),
            "decision": AuthorityDecision(row["decision"]),
            "approver_role": (
                HouseholdRole(row["approver_role"])
                if row["approver_role"]
                else HouseholdRole.owner
            ),
            "spending_limit": (
                float(row["spending_limit"])
                if row["spending_limit"] is not None
                else None
            ),
            "escalation_rule": row["escalation_rule"],
        }

    if capability == Capability.execute:
        return {
            "policy_id": None,
            "decision": AuthorityDecision.approval_required,
            "approver_role": HouseholdRole.owner,
            "spending_limit": None,
            "escalation_rule": "No execute policy exists.",
        }

    return {
        "policy_id": None,
        "decision": AuthorityDecision.allowed,
        "approver_role": None,
        "spending_limit": None,
        "escalation_rule": None,
    }


def request_approval(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    reason: str,
    requested_approver_role: HouseholdRole | object | None = HouseholdRole.owner,
) -> ApprovalRequest:
    role_value = (
        requested_approver_role.value
        if isinstance(requested_approver_role, HouseholdRole)
        else requested_approver_role
    )
    cursor.execute(
        """
        INSERT INTO approval_requests (
            workspace_id,
            work_order_id,
            requested_approver_role,
            reason
        )
        VALUES (%s, %s, %s, %s)
        RETURNING
            id,
            work_order_id,
            status,
            requested_approver_role,
            reason,
            decision_reason
        """,
        (workspace_id, work_order_id, role_value, reason),
    )
    row = cursor.fetchone()
    if row is None:
        raise RuntimeError("Approval request create did not return a saved row.")
    approval = _approval_request_from_row(row)
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="approval_request.created",
        entity_type="approval_request",
        entity_id=approval.id,
        metadata={
            "work_order_id": approval.work_order_id,
            "requested_approver_role": (
                approval.requested_approver_role.value
                if approval.requested_approver_role
                else None
            ),
            "reason": approval.reason,
        },
    )
    return approval


def approve_work_order(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    reason: str | None = None,
) -> ApprovalRequest | None:
    approval = _decide_latest_approval(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=ApprovalRequestStatus.approved,
        reason=reason,
    )
    _set_work_order_status(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=WorkOrderStatus.approved,
    )
    return approval


def reject_work_order(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    reason: str | None = None,
) -> ApprovalRequest | None:
    approval = _decide_latest_approval(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=ApprovalRequestStatus.rejected,
        reason=reason,
    )
    _set_work_order_status(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=WorkOrderStatus.rejected,
        result={"reason": reason} if reason else {},
    )
    return approval


def dismiss_work_order(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    reason: str | None = None,
) -> ApprovalRequest | None:
    approval = _decide_latest_approval(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=ApprovalRequestStatus.dismissed,
        reason=reason,
    )
    _set_work_order_status(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=WorkOrderStatus.dismissed,
        result={"reason": reason} if reason else {},
    )
    return approval


def mark_work_executed(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    result: Mapping[str, object] | None = None,
) -> WorkOrder:
    return _set_work_order_status(
        cursor,
        workspace_id=workspace_id,
        work_order_id=work_order_id,
        status=WorkOrderStatus.executed,
        result=dict(result or {}),
    )


def prepare_user_approved_work(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    request: WorkOrderCreate,
    approval_reason: str,
) -> WorkOrder:
    prepared = prepare_work(
        cursor,
        workspace_id=workspace_id,
        request=request,
        approval_reason=approval_reason,
    )
    if prepared.approval_request is not None:
        approve_work_order(
            cursor,
            workspace_id=workspace_id,
            work_order_id=prepared.work_order.id,
            reason="Approved by explicit user action.",
        )
    return prepared.work_order


def _decide_latest_approval(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    status: ApprovalRequestStatus,
    reason: str | None = None,
) -> ApprovalRequest | None:
    cursor.execute(
        """
        UPDATE approval_requests
        SET
            status = %s,
            decision_reason = %s,
            decided_at = now(),
            updated_at = now()
        WHERE id = (
            SELECT id
            FROM approval_requests
            WHERE workspace_id = %s
                AND work_order_id = %s
                AND status = 'pending'
            ORDER BY created_at DESC
            LIMIT 1
        )
        RETURNING
            id,
            work_order_id,
            status,
            requested_approver_role,
            reason,
            decision_reason
        """,
        (status.value, reason, workspace_id, work_order_id),
    )
    row = cursor.fetchone()
    if row is None:
        return None
    approval = _approval_request_from_row(row)
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type=f"approval_request.{status.value}",
        entity_type="approval_request",
        entity_id=approval.id,
        metadata={
            "work_order_id": approval.work_order_id,
            "decision_reason": approval.decision_reason,
        },
    )
    return approval


def _set_work_order_status(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    work_order_id: str,
    status: WorkOrderStatus,
    result: Mapping[str, object] | None = None,
) -> WorkOrder:
    cursor.execute(
        """
        UPDATE work_orders
        SET
            status = %s,
            result = result || %s::jsonb,
            updated_at = now()
        WHERE workspace_id = %s
            AND id = %s
        RETURNING
            id,
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
            result
        """,
        (status.value, Jsonb(dict(result or {})), workspace_id, work_order_id),
    )
    row = cursor.fetchone()
    if row is None:
        raise LookupError("Work order not found.")
    work_order = _work_order_from_row(row)
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type=f"work_order.{status.value}",
        entity_type="work_order",
        entity_id=work_order.id,
        metadata={
            "work_type": work_order.work_type,
            "status": work_order.status.value,
            "result_keys": list(work_order.result.keys()),
        },
    )
    return work_order


def _work_order_from_row(row: dict[str, Any]) -> WorkOrder:
    return WorkOrder(
        id=str(row["id"]),
        work_type=row["work_type"],
        title=row["title"],
        description=row["description"],
        status=WorkOrderStatus(row["status"]),
        capability_required=Capability(row["capability_required"]),
        subject_entity_type=row["subject_entity_type"],
        subject_entity_id=str(row["subject_entity_id"])
        if row["subject_entity_id"]
        else None,
        source_document_id=str(row["source_document_id"])
        if row["source_document_id"]
        else None,
        source_bill_id=str(row["source_bill_id"]) if row["source_bill_id"] else None,
        evidence=dict(row["evidence"] or {}),
        result=dict(row["result"] or {}),
    )


def _approval_request_from_row(row: dict[str, Any]) -> ApprovalRequest:
    return ApprovalRequest(
        id=str(row["id"]),
        work_order_id=str(row["work_order_id"]),
        status=ApprovalRequestStatus(row["status"]),
        requested_approver_role=(
            HouseholdRole(row["requested_approver_role"])
            if row["requested_approver_role"]
            else None
        ),
        reason=row["reason"],
        decision_reason=row["decision_reason"],
    )
