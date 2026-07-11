from datetime import date, datetime, time, timedelta
from typing import Any

from psycopg import Cursor
from psycopg.types.json import Jsonb

from app.services.audit import record_audit_event


def sync_bill_obligation(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    bill: dict[str, Any],
) -> dict[str, Any] | None:
    bill_id = bill["id"]
    due_date = bill["due_date"]
    status = _obligation_status_for_bill(bill)
    title = _obligation_title(bill)
    evidence = {
        "source": "bill",
        "bill_status": bill["status"],
        "bill_review_status": bill["review_status"],
        "document_id": str(bill["document_id"]) if bill["document_id"] else None,
    }
    cursor.execute(
        """
        SELECT id, status
        FROM obligations
        WHERE workspace_id = %s
            AND source_bill_id = %s
        """,
        (workspace_id, bill_id),
    )
    existing_obligation = cursor.fetchone()
    previous_status = existing_obligation["status"] if existing_obligation else None

    cursor.execute(
        """
        INSERT INTO obligations (
            workspace_id,
            source_bill_id,
            title,
            supplier,
            amount,
            currency,
            due_date,
            status,
            evidence
        )
        VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
        ON CONFLICT (workspace_id, source_bill_id)
        DO UPDATE SET
            title = EXCLUDED.title,
            supplier = EXCLUDED.supplier,
            amount = EXCLUDED.amount,
            currency = EXCLUDED.currency,
            due_date = EXCLUDED.due_date,
            status = EXCLUDED.status,
            evidence = obligations.evidence || EXCLUDED.evidence,
            updated_at = now()
        RETURNING
            id,
            source_bill_id,
            title,
            supplier,
            amount,
            currency,
            due_date,
            status,
            evidence,
            (xmax = 0) AS inserted
        """,
        (
            workspace_id,
            bill_id,
            title,
            bill["supplier"],
            bill["amount"],
            bill["currency"],
            due_date,
            status,
            Jsonb(evidence),
        ),
    )
    obligation = cursor.fetchone()
    if obligation is None:
        return None

    event_type = "obligation.created" if obligation["inserted"] else "obligation.updated"
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type=event_type,
        entity_type="obligation",
        entity_id=obligation["id"],
        metadata={
            "source_bill_id": str(bill_id),
            "status": obligation["status"],
            "due_date": obligation["due_date"].isoformat()
            if obligation["due_date"]
            else None,
        },
    )
    if (
        previous_status is not None
        and previous_status != obligation["status"]
    ):
        record_audit_event(
            cursor,
            workspace_id=workspace_id,
            event_type="obligation.status_changed",
            entity_type="obligation",
            entity_id=obligation["id"],
            metadata={
                "source_bill_id": str(bill_id),
                "previous_status": previous_status,
                "status": obligation["status"],
            },
        )

    if due_date and status not in {"paid", "archived"}:
        _ensure_obligation_reminder(
            cursor,
            workspace_id=workspace_id,
            obligation=obligation,
        )

    return obligation


def refresh_overdue_obligations(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[dict[str, Any]]:
    cursor.execute(
        """
        UPDATE obligations
        SET status = 'overdue', updated_at = now()
        WHERE workspace_id = %s
            AND status IN ('upcoming', 'due_soon')
            AND due_date < CURRENT_DATE
        RETURNING id, source_bill_id, title, status, due_date
        """,
        (workspace_id,),
    )
    rows = cursor.fetchall()
    for row in rows:
        record_audit_event(
            cursor,
            workspace_id=workspace_id,
            event_type="obligation.status_changed",
            entity_type="obligation",
            entity_id=row["id"],
            metadata={
                "source_bill_id": str(row["source_bill_id"])
                if row["source_bill_id"]
                else None,
                "status": row["status"],
                "due_date": row["due_date"].isoformat() if row["due_date"] else None,
            },
        )
    return rows


def refresh_overdue_obligations_for_workbench_read(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[dict[str, Any]]:
    """Prototype boundary until scheduled obligation maintenance is always running."""
    return refresh_overdue_obligations(cursor, workspace_id=workspace_id)


def backfill_confirmed_bill_obligations(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[dict[str, Any]]:
    cursor.execute(
        """
        SELECT
            b.id,
            b.document_id,
            b.supplier,
            b.amount,
            b.currency,
            b.due_date,
            b.status,
            b.review_status
        FROM bills b
        LEFT JOIN obligations o
            ON o.workspace_id = b.workspace_id
            AND o.source_bill_id = b.id
        WHERE b.workspace_id = %s
            AND b.review_status = 'confirmed'
            AND o.id IS NULL
        ORDER BY b.updated_at DESC
        """,
        (workspace_id,),
    )
    bill_rows = cursor.fetchall()
    obligations: list[dict[str, Any]] = []
    for bill in bill_rows:
        obligation = sync_bill_obligation(
            cursor,
            workspace_id=workspace_id,
            bill=bill,
        )
        if obligation is not None:
            obligations.append(obligation)

    if obligations:
        record_audit_event(
            cursor,
            workspace_id=workspace_id,
            event_type="obligation.backfill_completed",
            entity_type="obligation",
            entity_id=None,
            metadata={"created_or_updated_count": len(obligations)},
        )
    return obligations


def run_obligation_maintenance(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> dict[str, int]:
    backfilled = backfill_confirmed_bill_obligations(
        cursor,
        workspace_id=workspace_id,
    )
    overdue = refresh_overdue_obligations(
        cursor,
        workspace_id=workspace_id,
    )
    return {
        "backfilled_obligations": len(backfilled),
        "overdue_obligations": len(overdue),
    }


def _ensure_obligation_reminder(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    obligation: dict[str, Any],
) -> None:
    due_date = obligation["due_date"]
    if due_date is None:
        return

    reminder_at = datetime.combine(due_date, time(hour=9)) - timedelta(days=3)
    cursor.execute(
        """
        SELECT id
        FROM reminders
        WHERE workspace_id = %s
            AND related_entity_type = 'obligation'
            AND related_entity_id = %s
            AND status = 'scheduled'
        LIMIT 1
        """,
        (workspace_id, obligation["id"]),
    )
    existing = cursor.fetchone()
    if existing is not None:
        cursor.execute(
            """
            UPDATE reminders
            SET title = %s, remind_at = %s, updated_at = now()
            WHERE id = %s
            """,
            (
                f"{obligation['title']} is due soon",
                reminder_at,
                existing["id"],
            ),
        )
        return

    cursor.execute(
        """
        INSERT INTO reminders (
            workspace_id,
            title,
            remind_at,
            related_entity_type,
            related_entity_id
        )
        VALUES (%s, %s, %s, 'obligation', %s)
        RETURNING id
        """,
        (
            workspace_id,
            f"{obligation['title']} is due soon",
            reminder_at,
            obligation["id"],
        ),
    )
    reminder = cursor.fetchone()
    if reminder is None:
        return
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="reminder.created",
        entity_type="reminder",
        entity_id=reminder["id"],
        metadata={
            "related_entity_type": "obligation",
            "related_entity_id": str(obligation["id"]),
            "remind_at": reminder_at.isoformat(),
        },
    )


def _obligation_status_for_bill(bill: dict[str, Any]) -> str:
    if bill["status"] == "paid":
        return "paid"
    if bill["status"] == "archived":
        return "archived"
    if bill["review_status"] == "needs_review" or bill["status"] == "draft":
        return "needs_review"

    due_date = bill["due_date"]
    if due_date is None:
        return "needs_review"
    if due_date < date.today():
        return "overdue"
    if due_date <= date.today() + timedelta(days=3):
        return "due_soon"
    return "upcoming"


def _obligation_title(bill: dict[str, Any]) -> str:
    supplier = bill["supplier"]
    amount = bill["amount"]
    if amount is None:
        return f"{supplier} obligation"
    return f"{supplier} {float(amount):.2f} {bill['currency']}"
