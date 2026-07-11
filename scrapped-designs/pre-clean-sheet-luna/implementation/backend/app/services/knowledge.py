from app.db import get_connection
from app.models.knowledge import KnowledgeAskResponse, KnowledgeSource


def answer_household_question(question: str) -> KnowledgeAskResponse:
    normalized = " ".join(question.lower().split())

    if _asks_for_onboarding(normalized):
        return _answer_onboarding(question)
    if _asks_for_review(normalized):
        return _answer_needs_review(question)
    if _asks_for_paid(normalized):
        return _answer_paid(question)
    if _asks_for_due(normalized):
        return _answer_due(question)
    return _answer_search(question)


def _asks_for_onboarding(question: str) -> bool:
    return question in {"", "hello", "hi", "hey", "start", "help"} or any(
        phrase in question
        for phrase in (
            "who are you",
            "what do you do",
            "get started",
            "how do i start",
        )
    )


def _answer_onboarding(question: str) -> KnowledgeAskResponse:
    return KnowledgeAskResponse(
        question=question,
        answer=(
            "I'm Luna. I help manage household records, obligations, approvals, "
            "and admin work. To start, upload a bill or document and I'll prepare "
            "it for review."
        ),
        confidence=1.0,
        sources=[],
        suggested_next_actions=[
            "Open Records and upload a bill or document.",
            "Review Luna's approval request in the Workbench.",
            "Check Activity to see Luna's work history.",
        ],
    )


def _asks_for_due(question: str) -> bool:
    return any(term in question for term in ("due", "upcoming", "owe", "unpaid", "overdue"))


def _asks_for_review(question: str) -> bool:
    return any(term in question for term in ("review", "check", "uncertain", "missing", "attention"))


def _asks_for_paid(question: str) -> bool:
    return any(term in question for term in ("paid", "completed", "settled"))


def _answer_due(question: str) -> KnowledgeAskResponse:
    rows = _fetch_bills_by_status(("draft", "unpaid", "overdue"), limit=5)
    if not rows:
        return KnowledgeAskResponse(
            question=question,
            answer="I could not find any draft, unpaid, or overdue bills in Luna.",
            confidence=0.8,
            sources=[],
            suggested_next_actions=["Open Records and upload a bill or document for Luna to review."],
        )

    first = rows[0]
    answer = (
        f"{len(rows)} draft, unpaid, or overdue bill{'s' if len(rows) != 1 else ''} are visible. "
        f"The next one is {first['supplier']}"
    )
    if first["due_date"]:
        answer += f", due {first['due_date'].isoformat()}"
    if first["amount"] is not None:
        answer += f", for ${float(first['amount']):.2f}"
    answer += "."

    return KnowledgeAskResponse(
        question=question,
        answer=answer,
        confidence=0.82,
        sources=[_bill_source(row) for row in rows],
        suggested_next_actions=["Open the Workbench to review unpaid and overdue obligations."],
    )


def _answer_needs_review(question: str) -> KnowledgeAskResponse:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, title, description, due_date, related_entity_type, related_entity_id
                FROM tasks
                WHERE status = 'open'
                ORDER BY due_date NULLS LAST, created_at DESC
                LIMIT 5
                """
            )
            task_rows = cursor.fetchall()

            cursor.execute(
                """
                SELECT id, supplier, amount, due_date, review_reasons
                FROM bills
                WHERE review_status = 'needs_review'
                ORDER BY due_date NULLS LAST, created_at DESC
                LIMIT 5
                """
            )
            bill_rows = cursor.fetchall()

    if not task_rows and not bill_rows:
        return KnowledgeAskResponse(
            question=question,
            answer="I could not find open review tasks or bills marked as needing review.",
            confidence=0.8,
            sources=[],
        )

    sources = [
        KnowledgeSource(
            source_type="task",
            source_id=str(row["id"]),
            title=row["title"],
            detail=row["description"],
        )
        for row in task_rows
    ]
    sources.extend(_bill_source(row) for row in bill_rows)

    answer = (
        f"{len(task_rows)} open task{'s' if len(task_rows) != 1 else ''} "
        f"and {len(bill_rows)} bill{'s' if len(bill_rows) != 1 else ''} need review."
    )
    return KnowledgeAskResponse(
        question=question,
        answer=answer,
        confidence=0.84,
        sources=sources,
        suggested_next_actions=["Open the Workbench Needs review section."],
    )


def _answer_paid(question: str) -> KnowledgeAskResponse:
    rows = _fetch_bills_by_status(("paid",), limit=5)
    if not rows:
        return KnowledgeAskResponse(
            question=question,
            answer="I could not find bills marked as paid yet.",
            confidence=0.78,
            sources=[],
            suggested_next_actions=["Mark confirmed bills as paid once payment is verified."],
        )

    answer = f"{len(rows)} recently paid bill{'s' if len(rows) != 1 else ''} are recorded in Luna."
    return KnowledgeAskResponse(
        question=question,
        answer=answer,
        confidence=0.82,
        sources=[_bill_source(row) for row in rows],
    )


def _answer_search(question: str) -> KnowledgeAskResponse:
    terms = _search_terms(question)
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    d.id,
                    d.original_filename,
                    d.suggested_cabinet_path,
                    d.confirmed_cabinet_path,
                    b.supplier,
                    b.invoice_number,
                    b.category
                FROM documents d
                LEFT JOIN document_texts t ON t.document_id = d.id
                LEFT JOIN bills b ON b.document_id = d.id
                WHERE
                    d.original_filename ILIKE %s
                    OR d.suggested_cabinet_path ILIKE %s
                    OR d.confirmed_cabinet_path ILIKE %s
                    OR b.supplier ILIKE %s
                    OR b.invoice_number ILIKE %s
                    OR b.category ILIKE %s
                    OR t.text_content ILIKE %s
                ORDER BY d.received_at DESC
                LIMIT 5
                """,
                tuple([f"%{terms}%"] * 7),
            )
            rows = cursor.fetchall()

    if not rows:
        return _answer_onboarding(question)

    return KnowledgeAskResponse(
        question=question,
        answer=f"I found {len(rows)} matching document{'s' if len(rows) != 1 else ''}.",
        confidence=0.68,
        sources=[
            KnowledgeSource(
                source_type="document",
                source_id=str(row["id"]),
                title=row["original_filename"],
                detail=row["confirmed_cabinet_path"]
                or row["suggested_cabinet_path"]
                or row["supplier"],
            )
            for row in rows
        ],
        suggested_next_actions=["Open Records to inspect matching documents."],
    )


def _fetch_bills_by_status(statuses: tuple[str, ...], limit: int) -> list[dict[str, object]]:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT id, supplier, amount, due_date, status, review_reasons
                FROM bills
                WHERE status = ANY(%s)
                ORDER BY due_date NULLS LAST, created_at DESC
                LIMIT %s
                """,
                (list(statuses), limit),
            )
            return cursor.fetchall()


def _bill_source(row: dict[str, object]) -> KnowledgeSource:
    detail_parts: list[str] = []
    if row.get("due_date"):
        detail_parts.append(f"Due {row['due_date']}")
    if row.get("amount") is not None:
        detail_parts.append(f"${float(row['amount']):.2f}")
    if row.get("status"):
        detail_parts.append(str(row["status"]))
    review_reasons = row.get("review_reasons")
    if isinstance(review_reasons, list) and review_reasons:
        detail_parts.append("; ".join(str(reason) for reason in review_reasons))

    return KnowledgeSource(
        source_type="bill",
        source_id=str(row["id"]),
        title=str(row["supplier"]),
        detail=", ".join(detail_parts) or None,
    )


def _search_terms(question: str) -> str:
    stop_words = {
        "find",
        "show",
        "me",
        "where",
        "what",
        "is",
        "the",
        "for",
        "document",
        "documents",
        "bill",
        "invoice",
    }
    words = [
        word.strip(" ?.,:;!()[]{}")
        for word in question.split()
        if word.strip(" ?.,:;!()[]{}").lower() not in stop_words
    ]
    return " ".join(words) or question
