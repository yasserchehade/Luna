from fastapi import APIRouter
from psycopg.types.json import Jsonb

from app.db import get_connection
from app.models.bill import Bill, BillIngestRequest, BillIngestResponse, BillStatus
from app.services.extraction import get_extractor

router = APIRouter(prefix="/bills", tags=["bills"])


@router.get("", response_model=list[Bill])
def list_bills() -> list[Bill]:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    id,
                    document_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status
                FROM bills
                ORDER BY due_date NULLS LAST, created_at DESC
                """
            )
            rows = cursor.fetchall()

    return [
        Bill(
            id=str(row["id"]),
            document_id=str(row["document_id"]) if row["document_id"] else None,
            supplier=row["supplier"],
            amount=float(row["amount"]) if row["amount"] is not None else None,
            currency=row["currency"],
            due_date=row["due_date"].isoformat() if row["due_date"] else None,
            invoice_number=row["invoice_number"],
            category=row["category"],
            classification=row["classification"],
            status=BillStatus(row["status"]),
        )
        for row in rows
    ]


@router.post("/ingest", response_model=BillIngestResponse)
def ingest_bill(request: BillIngestRequest) -> BillIngestResponse:
    extractor = get_extractor()
    extracted = extractor.extract_from_document(request.document_id)

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
                from fastapi import HTTPException, status

                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail="Document not found.",
                )

            cursor.execute(
                """
                INSERT INTO extraction_runs (document_id, provider, confidence, output)
                VALUES (%s, %s, %s, %s)
                """,
                (
                    request.document_id,
                    "stub",
                    extracted.get("confidence"),
                    Jsonb(extracted),
                ),
            )

            cursor.execute(
                """
                INSERT INTO bills (
                    workspace_id,
                    document_id,
                    supplier,
                    amount,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING
                    id,
                    document_id,
                    supplier,
                    amount,
                    currency,
                    due_date,
                    invoice_number,
                    category,
                    classification,
                    status
                """,
                (
                    document["workspace_id"],
                    request.document_id,
                    extracted.get("supplier") or "Unknown supplier",
                    extracted.get("amount"),
                    extracted.get("due_date"),
                    extracted.get("invoice_number"),
                    extracted.get("category"),
                    extracted.get("classification"),
                    BillStatus.draft.value,
                ),
            )
            row = cursor.fetchone()

    if row is None:
        raise RuntimeError("Bill ingest did not return a saved bill.")

    bill = Bill(
        id=str(row["id"]),
        document_id=str(row["document_id"]) if row["document_id"] else None,
        supplier=row["supplier"],
        amount=float(row["amount"]) if row["amount"] is not None else None,
        currency=row["currency"],
        due_date=row["due_date"].isoformat() if row["due_date"] else None,
        invoice_number=row["invoice_number"],
        category=row["category"],
        classification=row["classification"],
        status=BillStatus(row["status"]),
    )
    return BillIngestResponse(document_id=request.document_id, bill=bill, extraction=extracted)
