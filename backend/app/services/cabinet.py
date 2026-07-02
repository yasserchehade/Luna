import re
from datetime import date
from pathlib import Path

from app.db import get_connection


MAX_SEGMENT_LENGTH = 80


def plan_document_cabinet_path(document_id: str) -> dict[str, object]:
    context = _load_document_context(document_id)
    if context is None:
        raise ValueError("Document not found.")

    bill = context["bill"]
    related_entities = context["related_entities"]
    reasons: list[str] = []

    root_segments = _graph_segments(related_entities)
    if root_segments:
        reasons.append("Used graph relationships to place the document.")
    else:
        root_segments = ["Inbox", "Needs Review"]
        reasons.append("No household graph placement was available.")

    category = _category_segment(bill)
    if category:
        root_segments.append(category)
        reasons.append("Used extracted bill category for the document folder.")

    filename = _suggested_filename(context["document"], bill)
    suggested_path = "/".join([*root_segments, filename])

    return {
        "document_id": document_id,
        "storage_provider": context["document"]["storage_provider"] or "local_folder",
        "cabinet_status": "suggested",
        "suggested_cabinet_path": suggested_path,
        "reasons": reasons,
    }


def save_document_cabinet_plan(document_id: str) -> dict[str, object]:
    plan = plan_document_cabinet_path(document_id)

    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                UPDATE documents
                SET
                    cabinet_status = %s,
                    suggested_cabinet_path = %s
                WHERE id = %s
                """,
                (
                    plan["cabinet_status"],
                    plan["suggested_cabinet_path"],
                    document_id,
                ),
            )

    return plan


def confirm_document_cabinet_path(
    document_id: str,
    cabinet_path: str | None = None,
) -> None:
    confirmed_path = _safe_cabinet_path(cabinet_path) if cabinet_path else None

    with get_connection() as connection:
        with connection.cursor() as cursor:
            if confirmed_path is None:
                cursor.execute(
                    """
                    SELECT suggested_cabinet_path
                    FROM documents
                    WHERE id = %s
                    """,
                    (document_id,),
                )
                row = cursor.fetchone()
                if row is None:
                    raise ValueError("Document not found.")
                confirmed_path = row["suggested_cabinet_path"]

            if not confirmed_path:
                raise ValueError("Document does not have a cabinet path to confirm.")

            cursor.execute(
                """
                UPDATE documents
                SET
                    cabinet_status = 'confirmed',
                    confirmed_cabinet_path = %s
                WHERE id = %s
                """,
                (confirmed_path, document_id),
            )
            if cursor.rowcount == 0:
                raise ValueError("Document not found.")


def _load_document_context(document_id: str) -> dict[str, object] | None:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    id,
                    original_filename,
                    storage_provider,
                    storage_path
                FROM documents
                WHERE id = %s
                """,
                (document_id,),
            )
            document = cursor.fetchone()
            if document is None:
                return None

            cursor.execute(
                """
                SELECT
                    supplier,
                    amount,
                    due_date,
                    invoice_number,
                    category,
                    classification
                FROM bills
                WHERE document_id = %s
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (document_id,),
            )
            bill = cursor.fetchone()

            cursor.execute(
                """
                SELECT DISTINCT
                    he.entity_type,
                    he.display_name
                FROM entity_relationships er
                JOIN household_entities he
                    ON he.id = er.source_entity_id
                    OR he.id = er.target_entity_id
                WHERE er.provenance_document_id = %s
                    OR (
                        er.source_entity_type = 'document'
                        AND er.source_entity_id = %s
                    )
                    OR (
                        er.target_entity_type = 'document'
                        AND er.target_entity_id = %s
                    )
                ORDER BY he.entity_type, he.display_name
                """,
                (document_id, document_id, document_id),
            )
            related_entities = cursor.fetchall()

    return {
        "document": document,
        "bill": bill,
        "related_entities": related_entities,
    }


def _graph_segments(entities: list[dict[str, object]]) -> list[str]:
    preferred_order = [
        "family_trust",
        "business",
        "property",
        "vehicle",
        "family_member",
        "supplier",
    ]
    grouped: dict[str, list[str]] = {}
    for entity in entities:
        entity_type = str(entity["entity_type"])
        grouped.setdefault(entity_type, []).append(str(entity["display_name"]))

    segments: list[str] = []
    for entity_type in preferred_order:
        names = grouped.get(entity_type)
        if not names:
            continue
        label = _entity_group_label(entity_type)
        segments.extend([label, _safe_segment(names[0])])

    return segments


def _entity_group_label(entity_type: str) -> str:
    labels = {
        "family_trust": "Trusts",
        "business": "Businesses",
        "property": "Properties",
        "vehicle": "Vehicles",
        "family_member": "Family Members",
        "supplier": "Suppliers",
    }
    return labels.get(entity_type, _safe_segment(entity_type.replace("_", " ").title()))


def _category_segment(bill: dict[str, object] | None) -> str | None:
    if bill is None:
        return "Documents"

    category = bill["category"]
    if category:
        return _safe_segment(str(category).replace("_", " ").title())
    return "Bills"


def _suggested_filename(
    document: dict[str, object],
    bill: dict[str, object] | None,
) -> str:
    original = Path(str(document["original_filename"]))
    extension = original.suffix.lower() or ".pdf"

    if bill is None:
        return f"{_safe_segment(original.stem)}{extension}"

    pieces: list[str] = []
    due_date = bill["due_date"]
    if isinstance(due_date, date):
        pieces.append(due_date.isoformat())
    supplier = bill["supplier"]
    if supplier:
        pieces.append(str(supplier))
    invoice_number = bill["invoice_number"]
    if invoice_number:
        pieces.append(f"Invoice-{invoice_number}")
    amount = bill["amount"]
    if amount is not None:
        pieces.append(f"{float(amount):.2f}")

    if not pieces:
        pieces.append(original.stem)

    return f"{_safe_segment('_'.join(pieces))}{extension}"


def _safe_segment(value: str) -> str:
    cleaned = re.sub(r"[<>:\"/\\|?*\x00-\x1f]+", " ", value)
    cleaned = re.sub(r"\s+", " ", cleaned).strip(" .")
    cleaned = cleaned.replace(" ", "-")
    cleaned = re.sub(r"-+", "-", cleaned)
    return (cleaned or "Unsorted")[:MAX_SEGMENT_LENGTH]


def _safe_cabinet_path(value: str) -> str:
    segments = [
        _safe_segment(segment)
        for segment in re.split(r"[/\\]+", value)
        if segment.strip()
    ]
    if not segments:
        return ""
    return "/".join(segments)
