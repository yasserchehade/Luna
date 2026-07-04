import re
import shutil
from datetime import date
from pathlib import Path

from app.core.config import settings
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


def file_document_in_cabinet(document_id: str, mode: str = "copy") -> dict[str, object]:
    normalized_mode = mode.strip().lower()
    if normalized_mode not in {"copy", "move"}:
        raise ValueError("Filing mode must be copy or move.")

    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT storage_path, confirmed_cabinet_path, cabinet_status
                FROM documents
                WHERE id = %s
                """,
                (document_id,),
            )
            row = cursor.fetchone()
            if row is None:
                raise ValueError("Document not found.")
            if row["cabinet_status"] != "confirmed":
                raise ValueError("Document must have a confirmed cabinet path before filing.")
            if not row["confirmed_cabinet_path"]:
                raise ValueError("Document does not have a confirmed cabinet path.")

            source_path = Path(str(row["storage_path"]))
            if not source_path.exists() or not source_path.is_file():
                raise ValueError("Document source file was not found.")

            destination = _cabinet_destination_path(str(row["confirmed_cabinet_path"]))
            destination.parent.mkdir(parents=True, exist_ok=True)
            filed_path = _unique_destination_path(destination)

            if normalized_mode == "move":
                shutil.move(str(source_path), str(filed_path))
            else:
                shutil.copy2(source_path, filed_path)

            cursor.execute(
                """
                UPDATE documents
                SET
                    cabinet_status = 'filed',
                    storage_path = %s
                WHERE id = %s
                """,
                (str(filed_path), document_id),
            )

    return {
        "document_id": document_id,
        "source_path": str(source_path),
        "filed_path": str(filed_path),
        "mode": normalized_mode,
    }


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
    cleaned = cleaned.replace("-.", ".").strip("-")
    return (cleaned or "Unsorted")[:MAX_SEGMENT_LENGTH]


def _safe_cabinet_path(value: str) -> str:
    segments = [
        _safe_segment(segment)
        for segment in re.split(r"[/\\]+", value)
        if segment.strip() and segment.strip(" .") not in {"", "."}
    ]
    if not segments:
        return ""
    return "/".join(segments)


def _cabinet_destination_path(cabinet_path: str) -> Path:
    root = Path(settings.cabinet_storage_path).resolve()
    relative_path = Path(*_safe_cabinet_path(cabinet_path).split("/"))
    destination = (root / relative_path).resolve()

    if root != destination and root not in destination.parents:
        raise ValueError("Cabinet path escapes the configured cabinet root.")
    return destination


def _unique_destination_path(destination: Path) -> Path:
    if not destination.exists():
        return destination

    stem = destination.stem
    suffix = destination.suffix
    parent = destination.parent
    for counter in range(2, 1000):
        candidate = parent / f"{stem}-{counter}{suffix}"
        if not candidate.exists():
            return candidate

    raise ValueError("Could not find a safe cabinet filename.")
