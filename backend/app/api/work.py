from fastapi import APIRouter, HTTPException, status

from app.db import get_connection, get_default_workspace_id
from app.models.work import (
    ApprovalDecisionRequest,
    ApprovalDecisionResponse,
    ApprovalRequest,
    ApprovalRequestStatus,
    Capability,
    HouseholdRole,
    WorkOrder,
    WorkOrderStatus,
)
from app.services.work import approve_work_order, reject_work_order

router = APIRouter(prefix="/work", tags=["work"])


def _work_order_from_row(row) -> WorkOrder:
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


def _approval_from_row(row) -> ApprovalRequest:
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


@router.get("/orders", response_model=list[WorkOrder])
def list_work_orders() -> list[WorkOrder]:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
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
                FROM work_orders
                WHERE workspace_id = %s
                ORDER BY created_at DESC
                LIMIT 50
                """,
                (workspace_id,),
            )
            rows = cursor.fetchall()

    return [_work_order_from_row(row) for row in rows]


@router.get("/approvals", response_model=list[ApprovalRequest])
def list_approval_requests() -> list[ApprovalRequest]:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    id,
                    work_order_id,
                    status,
                    requested_approver_role,
                    reason,
                    decision_reason
                FROM approval_requests
                WHERE workspace_id = %s
                ORDER BY created_at DESC
                LIMIT 50
                """,
                (workspace_id,),
            )
            rows = cursor.fetchall()

    return [_approval_from_row(row) for row in rows]


@router.post(
    "/approvals/{approval_id}/approve",
    response_model=ApprovalDecisionResponse,
)
def approve_request(
    approval_id: str,
    request: ApprovalDecisionRequest | None = None,
) -> ApprovalDecisionResponse:
    return _decide_approval(
        approval_id,
        decision=ApprovalRequestStatus.approved,
        reason=request.reason if request else None,
    )


@router.post(
    "/approvals/{approval_id}/reject",
    response_model=ApprovalDecisionResponse,
)
def reject_request(
    approval_id: str,
    request: ApprovalDecisionRequest | None = None,
) -> ApprovalDecisionResponse:
    return _decide_approval(
        approval_id,
        decision=ApprovalRequestStatus.rejected,
        reason=request.reason if request else None,
    )


def _decide_approval(
    approval_id: str,
    *,
    decision: ApprovalRequestStatus,
    reason: str | None,
) -> ApprovalDecisionResponse:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT work_order_id
                FROM approval_requests
                WHERE workspace_id = %s
                    AND id = %s
                """,
                (workspace_id, approval_id),
            )
            approval_row = cursor.fetchone()
            if approval_row is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Approval request not found.",
                )

            if decision == ApprovalRequestStatus.approved:
                approval = approve_work_order(
                    cursor,
                    workspace_id=workspace_id,
                    work_order_id=str(approval_row["work_order_id"]),
                    reason=reason,
                )
            else:
                approval = reject_work_order(
                    cursor,
                    workspace_id=workspace_id,
                    work_order_id=str(approval_row["work_order_id"]),
                    reason=reason,
                )

            cursor.execute(
                """
                SELECT
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
                FROM work_orders
                WHERE workspace_id = %s
                    AND id = %s
                """,
                (workspace_id, approval_row["work_order_id"]),
            )
            work_row = cursor.fetchone()
            if approval is None or work_row is None:
                raise HTTPException(
                    status_code=status.HTTP_409_CONFLICT,
                    detail="Approval request is not pending.",
                )

    return ApprovalDecisionResponse(
        approval_request=approval,
        work_order=_work_order_from_row(work_row),
    )
