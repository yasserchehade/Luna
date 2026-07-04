from datetime import datetime, time, timedelta
from uuid import UUID

from fastapi import APIRouter, HTTPException, status
from psycopg.types.json import Jsonb

from app.db import get_connection
from app.models.bill import (
    Bill,
    BillActionResponse,
    BillIngestRequest,
    BillIngestResponse,
    BillReviewStatus,
    BillStatus,
    BillUpdate,
)
from app.services.audit import record_audit_event
from app.services.cabinet import save_document_cabinet_plan
from app.services.extraction import get_extractor
from app.services.supplier_profiles import record_supplier_template_match

router = APIRouter(prefix="/bills", tags=["bills"])


def _bill_from_row(row) -> Bill:
    return Bill(
        id=str(row["id"]),
        document_id=str(row["document_id"]) if row["document_id"] else None,
        supplier=row["supplier"],
        supplier_entity_id=str(row["supplier_entity_id"]) if row["supplier_entity_id"] else None,
        amount=float(row["amount"]) if row["amount"] is not None else None,
        currency=row["currency"],
        due_date=row["due_date"].isoformat() if row["due_date"] else None,
        invoice_number=row["invoice_number"],
        category=row["category"],
        classification=row["classification"],
        status=BillStatus(row["status"]),
        extraction_confidence=(
            float(row["extraction_confidence"])
            if row["extraction_confidence"] is not None
            else None
        ),
        review_status=BillReviewStatus(row["review_status"]),
        review_reasons=list(row["review_reasons"] or []),
    )


def _get_or_create_supplier_entity(cursor, workspace_id: UUID, supplier: str) -> UUID:
    cursor.execute(
        """
        SELECT id
        FROM household_entities
        WHERE workspace_id = %s
            AND entity_type = 'supplier'
            AND lower(display_name) = lower(%s)
        LIMIT 1
        """,
        (workspace_id, supplier),
    )
    row = cursor.fetchone()
    if row:
        return row["id"]

    cursor.execute(
        """
        INSERT INTO household_entities (workspace_id, entity_type, display_name)
        VALUES (%s, 'supplier', %s)
        RETURNING id
        """,
        (workspace_id, supplier),
    )
    created = cursor.fetchone()
    if created is None:
        raise RuntimeError("Could not create supplier entity.")
    return created["id"]


def _build_review_reasons(extracted: dict[str, object]) -> list[str]:
    reasons: list[str] = []
    supplier = str(extracted.get("supplier") or "")
    confidence = extracted.get("confidence")

    if not supplier or supplier == "Unknown supplier":
        reasons.append("Supplier could not be identified.")
    if extracted.get("amount") is None:
        reasons.append("Amount could not be confidently extracted.")
    if extracted.get("due_date") is None:
        reasons.append("Due date could not be confidently extracted.")
    if isinstance(confidence, (int, float)) and confidence < 0.75:
        reasons.append("Extraction confidence is below the review threshold.")

    supplier_profile = extracted.get("supplier_profile")
    if isinstance(supplier_profile, dict):
        template_status = supplier_profile.get("template_status")
        if template_status in {"changed", "needs_review"}:
            reasons.append("Known supplier template appears to have changed.")
        elif template_status == "unknown":
            reasons.append("Supplier template has not been reviewed before.")

    return reasons


def _review_status_from_reasons(reasons: list[str]) -> BillReviewStatus:
    return BillReviewStatus.needs_review if reasons else BillReviewStatus.not_required


@router.get("", response_model=list[Bill])
def list_bills() -> list[Bill]:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                FROM bills
                ORDER BY due_date NULLS LAST, created_at DESC
                """
            )
            rows = cursor.fetchall()

    return [_bill_from_row(row) for row in rows]


@router.post("/ingest", response_model=BillIngestResponse)
def ingest_bill(request: BillIngestRequest) -> BillIngestResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT workspace_id
                FROM documents
                WHERE id = %s
                """,
                (request.document_id,),
            )
            document = cursor.fetchone()
            if document is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Document not found.",
                )

            cursor.execute(
                """
                SELECT
                    id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                FROM bills
                WHERE document_id = %s
                ORDER BY created_at
                LIMIT 1
                """,
                (request.document_id,),
            )
            existing_bill = cursor.fetchone()
            if existing_bill is not None:
                return BillIngestResponse(
                    document_id=request.document_id,
                    bill=_bill_from_row(existing_bill),
                    extraction={
                        "provider": "existing_record",
                        "confidence": None,
                        "notes": "Document already has an associated bill.",
                    },
                )

            extractor = get_extractor()
            extracted = extractor.extract_from_document(request.document_id)
            supplier = str(extracted.get("supplier") or "Unknown supplier")
            review_reasons = _build_review_reasons(extracted)
            review_status = _review_status_from_reasons(review_reasons)
            supplier_entity_id = _get_or_create_supplier_entity(
                cursor,
                document["workspace_id"],
                supplier,
            )

            cursor.execute(
                """
                INSERT INTO extraction_runs (document_id, provider, confidence, output)
                VALUES (%s, %s, %s, %s)
                """,
                (
                    request.document_id,
                    str(extracted.get("provider") or "local_rules"),
                    extracted.get("confidence"),
                    Jsonb(extracted),
                ),
            )
            record_supplier_template_match(
                cursor,
                workspace_id=document["workspace_id"],
                supplier_entity_id=supplier_entity_id,
                document_id=request.document_id,
                extraction=extracted,
            )

            cursor.execute(
                """
                INSERT INTO bills (
                    workspace_id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING
                    id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                """,
                (
                    document["workspace_id"],
                    request.document_id,
                    supplier_entity_id,
                    supplier,
                    extracted.get("amount"),
                    extracted.get("due_date"),
                    extracted.get("invoice_number"),
                    extracted.get("category"),
                    extracted.get("classification"),
                    BillStatus.draft.value,
                    extracted.get("confidence"),
                    review_status.value,
                    Jsonb(review_reasons),
                ),
            )
            row = cursor.fetchone()
            if row is None:
                raise RuntimeError("Bill ingest did not return a saved bill.")
            record_audit_event(
                cursor,
                workspace_id=document["workspace_id"],
                event_type="bill.ingested",
                entity_type="bill",
                entity_id=row["id"],
                metadata={
                    "document_id": request.document_id,
                    "supplier": row["supplier"],
                    "review_status": row["review_status"],
                    "extraction_confidence": (
                        float(row["extraction_confidence"])
                        if row["extraction_confidence"] is not None
                        else None
                    ),
                },
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
                VALUES (%s, 'document', %s, 'issued_by', 'supplier', %s, %s, %s)
                """,
                (
                    document["workspace_id"],
                    request.document_id,
                    supplier_entity_id,
                    request.document_id,
                    extracted.get("confidence"),
                ),
            )

            if row["due_date"]:
                reminder_at = datetime.combine(row["due_date"], time(hour=9)) - timedelta(days=3)
                cursor.execute(
                    """
                    INSERT INTO reminders (
                        workspace_id,
                        title,
                        remind_at,
                        related_entity_type,
                        related_entity_id
                    )
                    VALUES (%s, %s, %s, 'bill', %s)
                    """,
                    (
                        document["workspace_id"],
                        f"Review {row['supplier']} bill before it is due",
                        reminder_at,
                        row["id"],
                    ),
                )

            if review_reasons:
                cursor.execute(
                    """
                    INSERT INTO tasks (
                        workspace_id,
                        title,
                        description,
                        related_entity_type,
                        related_entity_id
                    )
                    VALUES (%s, %s, %s, 'bill', %s)
                    """,
                    (
                        document["workspace_id"],
                        f"Review extracted details for {row['supplier']}",
                        " ".join(review_reasons),
                        row["id"],
                    ),
                )

    bill = _bill_from_row(row)
    cabinet_plan = save_document_cabinet_plan(request.document_id)
    extraction_with_cabinet = {
        **extracted,
        "cabinet_plan": cabinet_plan,
    }
    return BillIngestResponse(
        document_id=request.document_id,
        bill=bill,
        extraction=extraction_with_cabinet,
    )


@router.patch("/{bill_id}", response_model=BillActionResponse)
def update_bill(bill_id: str, update: BillUpdate) -> BillActionResponse:
    fields = update.model_dump(exclude_unset=True)
    if not fields:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="At least one bill field must be provided.",
        )

    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT workspace_id, supplier
                FROM bills
                WHERE id = %s
                """,
                (bill_id,),
            )
            existing = cursor.fetchone()
            if existing is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Bill not found.",
                )

            supplier_entity_id = None
            if "supplier" in fields:
                supplier_entity_id = _get_or_create_supplier_entity(
                    cursor,
                    existing["workspace_id"],
                    fields["supplier"],
                )

            cursor.execute(
                """
                UPDATE bills
                SET
                    supplier = COALESCE(%s, supplier),
                    supplier_entity_id = COALESCE(%s, supplier_entity_id),
                    amount = COALESCE(%s, amount),
                    due_date = COALESCE(%s, due_date),
                    invoice_number = COALESCE(%s, invoice_number),
                    category = COALESCE(%s, category),
                    classification = COALESCE(%s, classification),
                    updated_at = now()
                WHERE id = %s
                RETURNING
                    id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                """,
                (
                    fields.get("supplier"),
                    supplier_entity_id,
                    fields.get("amount"),
                    fields.get("due_date"),
                    fields.get("invoice_number"),
                    fields.get("category"),
                    fields.get("classification"),
                    bill_id,
                ),
            )
            row = cursor.fetchone()
            if row is not None:
                record_audit_event(
                    cursor,
                    workspace_id=existing["workspace_id"],
                    event_type="bill.updated",
                    entity_type="bill",
                    entity_id=row["id"],
                    metadata={
                        "updated_fields": sorted(fields.keys()),
                        "supplier": row["supplier"],
                    },
                )

    if row is None:
        raise RuntimeError("Bill update did not return a saved bill.")
    return BillActionResponse(bill=_bill_from_row(row))


@router.post("/{bill_id}/confirm", response_model=BillActionResponse)
def confirm_bill(bill_id: str) -> BillActionResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                UPDATE bills
                SET
                    status = %s,
                    review_status = %s,
                    review_reasons = '[]'::jsonb,
                    updated_at = now()
                WHERE id = %s
                RETURNING
                    id,
                    workspace_id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                """,
                (BillStatus.unpaid.value, BillReviewStatus.confirmed.value, bill_id),
            )
            row = cursor.fetchone()
            if row is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Bill not found.",
                )
            record_audit_event(
                cursor,
                workspace_id=row["workspace_id"],
                event_type="bill.confirmed",
                entity_type="bill",
                entity_id=row["id"],
                metadata={"supplier": row["supplier"], "status": row["status"]},
            )

            cursor.execute(
                """
                UPDATE tasks
                SET status = 'done', updated_at = now()
                WHERE related_entity_type = 'bill'
                    AND related_entity_id = %s
                    AND status = 'open'
                """,
                (bill_id,),
            )

    return BillActionResponse(bill=_bill_from_row(row))


@router.post("/{bill_id}/mark-paid", response_model=BillActionResponse)
def mark_bill_paid(bill_id: str) -> BillActionResponse:
    return _set_bill_status(bill_id, BillStatus.paid)


@router.post("/{bill_id}/archive", response_model=BillActionResponse)
def archive_bill(bill_id: str) -> BillActionResponse:
    return _set_bill_status(bill_id, BillStatus.archived)


def _set_bill_status(bill_id: str, bill_status: BillStatus) -> BillActionResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                UPDATE bills
                SET status = %s, updated_at = now()
                WHERE id = %s
                RETURNING
                    id,
                    workspace_id,
                    document_id,
                    supplier_entity_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status,
                    extraction_confidence,
                    review_status,
                    review_reasons
                """,
                (bill_status.value, bill_id),
            )
            row = cursor.fetchone()
            if row is None:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Bill not found.",
                )
            record_audit_event(
                cursor,
                workspace_id=row["workspace_id"],
                event_type=f"bill.{bill_status.value}",
                entity_type="bill",
                entity_id=row["id"],
                metadata={"supplier": row["supplier"], "status": row["status"]},
            )

            if bill_status in {BillStatus.unpaid, BillStatus.paid, BillStatus.archived}:
                cursor.execute(
                    """
                    UPDATE tasks
                    SET status = 'done', updated_at = now()
                    WHERE related_entity_type = 'bill'
                        AND related_entity_id = %s
                        AND status = 'open'
                    """,
                    (bill_id,),
                )

    return BillActionResponse(bill=_bill_from_row(row))
