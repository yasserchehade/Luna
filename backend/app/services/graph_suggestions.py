from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Any

from psycopg import Cursor
from psycopg.types.json import Jsonb

from app.models.household import GraphSuggestion, GraphSuggestionAction, GraphSuggestionStatus
from app.services.audit import record_audit_event


UTILITY_CATEGORIES = {
    "electricity": "Electricity Account",
    "gas": "Gas Account",
    "internet": "Internet Account",
    "telecommunications": "Internet Account",
    "water": "Water Account",
    "utility": "Utility Account",
    "utilities": "Utility Account",
}


@dataclass(frozen=True)
class SuggestionCandidate:
    action_type: str
    action_payload: dict[str, object]
    confidence: float
    reasoning: str
    affected_entities: list[dict[str, object]] = field(default_factory=list)
    source_document_id: str | None = None
    source_bill_id: str | None = None

    @property
    def fingerprint(self) -> str:
        payload = {
            "action_payload": self.action_payload,
            "action_type": self.action_type,
            "source_bill_id": self.source_bill_id,
            "source_document_id": self.source_document_id,
        }
        encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"))
        return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def refresh_pending_graph_suggestions(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> None:
    ensure_graph_suggestion_schema(cursor)
    for candidate in _build_candidates(cursor, workspace_id=workspace_id):
        cursor.execute(
            """
            INSERT INTO graph_suggestions (
                workspace_id,
                fingerprint,
                action_type,
                action_payload,
                confidence,
                reasoning,
                affected_entities,
                source_document_id,
                source_bill_id
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (workspace_id, fingerprint) DO NOTHING
            """,
            (
                workspace_id,
                candidate.fingerprint,
                candidate.action_type,
                Jsonb(candidate.action_payload),
                candidate.confidence,
                candidate.reasoning,
                Jsonb(candidate.affected_entities),
                candidate.source_document_id,
                candidate.source_bill_id,
            ),
        )


def list_pending_graph_suggestions(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[GraphSuggestion]:
    ensure_graph_suggestion_schema(cursor)
    refresh_pending_graph_suggestions(cursor, workspace_id=workspace_id)
    cursor.execute(
        """
        SELECT
            id,
            confidence,
            action_type,
            reasoning,
            affected_entities,
            status,
            action_payload,
            source_document_id,
            source_bill_id
        FROM graph_suggestions
        WHERE workspace_id = %s
            AND status = 'pending'
        ORDER BY confidence DESC, created_at DESC
        """,
        (workspace_id,),
    )
    return [_suggestion_from_row(row) for row in cursor.fetchall()]


def accept_graph_suggestion(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    suggestion_id: str,
) -> GraphSuggestion:
    ensure_graph_suggestion_schema(cursor)
    row = _get_pending_suggestion(cursor, workspace_id=workspace_id, suggestion_id=suggestion_id)
    _apply_suggestion(cursor, workspace_id=workspace_id, row=row)
    cursor.execute(
        """
        UPDATE graph_suggestions
        SET status = 'accepted', updated_at = now(), decided_at = now()
        WHERE workspace_id = %s
            AND id = %s
        RETURNING
            id,
            confidence,
            action_type,
            reasoning,
            affected_entities,
            status,
            action_payload,
            source_document_id,
            source_bill_id
        """,
        (workspace_id, suggestion_id),
    )
    updated = cursor.fetchone()
    if updated is None:
        raise RuntimeError("Graph suggestion accept did not return a suggestion.")
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="graph_suggestion.accepted",
        entity_type="graph_suggestion",
        entity_id=updated["id"],
        metadata={
            "action_type": updated["action_type"],
            "reasoning": updated["reasoning"],
            "source_document_id": (
                str(updated["source_document_id"]) if updated["source_document_id"] else None
            ),
            "source_bill_id": str(updated["source_bill_id"]) if updated["source_bill_id"] else None,
        },
    )
    return _suggestion_from_row(updated)


def reject_graph_suggestion(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    suggestion_id: str,
) -> GraphSuggestion:
    ensure_graph_suggestion_schema(cursor)
    _get_pending_suggestion(cursor, workspace_id=workspace_id, suggestion_id=suggestion_id)
    cursor.execute(
        """
        UPDATE graph_suggestions
        SET status = 'rejected', updated_at = now(), decided_at = now()
        WHERE workspace_id = %s
            AND id = %s
        RETURNING
            id,
            confidence,
            action_type,
            reasoning,
            affected_entities,
            status,
            action_payload,
            source_document_id,
            source_bill_id
        """,
        (workspace_id, suggestion_id),
    )
    updated = cursor.fetchone()
    if updated is None:
        raise RuntimeError("Graph suggestion reject did not return a suggestion.")
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="graph_suggestion.rejected",
        entity_type="graph_suggestion",
        entity_id=updated["id"],
        metadata={
            "action_type": updated["action_type"],
            "reasoning": updated["reasoning"],
            "fingerprint_preserved": True,
        },
    )
    return _suggestion_from_row(updated)


def ensure_graph_suggestion_schema(cursor: Cursor[dict[str, Any]]) -> None:
    cursor.execute(
        """
        CREATE TABLE IF NOT EXISTS graph_suggestions (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            fingerprint TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
            action_type TEXT NOT NULL CHECK (
                action_type IN (
                    'create_entity',
                    'connect_entities',
                    'update_metadata',
                    'attach_document',
                    'merge_duplicate_entities'
                )
            ),
            action_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
            confidence NUMERIC(4, 3) NOT NULL,
            reasoning TEXT NOT NULL,
            affected_entities JSONB NOT NULL DEFAULT '[]'::jsonb,
            source_document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
            source_bill_id UUID REFERENCES bills(id) ON DELETE SET NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            decided_at TIMESTAMPTZ,
            UNIQUE (workspace_id, fingerprint)
        )
        """
    )
    cursor.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_graph_suggestions_workspace_status
            ON graph_suggestions(workspace_id, status, created_at DESC)
        """
    )
    cursor.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_graph_suggestions_document
            ON graph_suggestions(workspace_id, source_document_id)
        """
    )
    cursor.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_graph_suggestions_bill
            ON graph_suggestions(workspace_id, source_bill_id)
        """
    )


def _build_candidates(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[SuggestionCandidate]:
    entities = _load_entities(cursor, workspace_id=workspace_id)
    relationships = _load_relationship_keys(cursor, workspace_id=workspace_id)
    documents = _load_documents(cursor, workspace_id=workspace_id)
    bills = _load_bills(cursor, workspace_id=workspace_id)

    candidates: list[SuggestionCandidate] = []
    for bill in bills:
        supplier_id = str(bill["supplier_entity_id"]) if bill["supplier_entity_id"] else None
        document_id = str(bill["document_id"]) if bill["document_id"] else None
        bill_id = str(bill["id"])

        if supplier_id and not _relationship_exists(
            relationships,
            source_type="bill",
            source_id=bill_id,
            relationship_type="issued_by",
            target_type="supplier",
            target_id=supplier_id,
        ):
            candidates.append(
                SuggestionCandidate(
                    action_type="connect_entities",
                    action_payload={
                        "source_entity_type": "bill",
                        "source_entity_id": bill_id,
                        "relationship_type": "issued_by",
                        "target_entity_type": "supplier",
                        "target_entity_id": supplier_id,
                        "provenance_document_id": document_id,
                    },
                    confidence=_confidence(bill, 0.88),
                    reasoning="The bill extraction identified this supplier.",
                    affected_entities=[
                        _affected("bill", bill_id, _bill_name(bill)),
                        _affected("supplier", supplier_id, str(bill["supplier"])),
                    ],
                    source_bill_id=bill_id,
                    source_document_id=document_id,
                )
            )

        if document_id:
            document = documents.get(document_id)
            property_entity = _match_property_for_document(document, entities)
            if property_entity and not _relationship_exists(
                relationships,
                source_type="document",
                source_id=document_id,
                relationship_type="concerns",
                target_type=property_entity["entity_type"],
                target_id=str(property_entity["id"]),
            ):
                candidates.append(
                    SuggestionCandidate(
                        action_type="attach_document",
                        action_payload={
                            "source_entity_type": "document",
                            "source_entity_id": document_id,
                            "relationship_type": "concerns",
                            "target_entity_type": property_entity["entity_type"],
                            "target_entity_id": str(property_entity["id"]),
                            "provenance_document_id": document_id,
                        },
                        confidence=0.86 if _document_mentions_entity(document, property_entity) else 0.68,
                        reasoning="The document appears to relate to this household property.",
                        affected_entities=[
                            _affected("document", document_id, _document_name(document, document_id)),
                            _affected(
                                property_entity["entity_type"],
                                str(property_entity["id"]),
                                property_entity["display_name"],
                            ),
                        ],
                        source_bill_id=bill_id,
                        source_document_id=document_id,
                    )
                )

        account_name = _utility_account_name(bill)
        if account_name and supplier_id and not _entity_exists(
            entities,
            entity_type="utility_account",
            display_name=account_name,
        ):
            payload: dict[str, object] = {
                "entity_type": "utility_account",
                "display_name": account_name,
                "metadata": {
                    "supplier": bill["supplier"],
                    "source_bill_id": bill_id,
                    "source_document_id": document_id,
                },
            }
            property_entity = _match_property_for_document(documents.get(document_id or ""), entities)
            if property_entity:
                payload["relationship"] = {
                    "source_entity_type": "utility_account",
                    "relationship_type": "services",
                    "target_entity_type": property_entity["entity_type"],
                    "target_entity_id": str(property_entity["id"]),
                    "provenance_document_id": document_id,
                }
            candidates.append(
                SuggestionCandidate(
                    action_type="create_entity",
                    action_payload=payload,
                    confidence=0.72,
                    reasoning="The bill category suggests a reusable utility account node.",
                    affected_entities=[
                        _affected("supplier", supplier_id, str(bill["supplier"])),
                        *(
                            [
                                _affected(
                                    property_entity["entity_type"],
                                    str(property_entity["id"]),
                                    property_entity["display_name"],
                                )
                            ]
                            if property_entity
                            else []
                        ),
                    ],
                    source_bill_id=bill_id,
                    source_document_id=document_id,
                )
            )

    return candidates


def _apply_suggestion(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    row: dict[str, Any],
) -> None:
    payload = dict(row["action_payload"] or {})
    payload.setdefault("confidence", float(row["confidence"]))
    action_type = row["action_type"]

    if action_type in {"connect_entities", "attach_document"}:
        _create_relationship_from_payload(cursor, workspace_id=workspace_id, payload=payload)
        return

    if action_type == "create_entity":
        cursor.execute(
            """
            SELECT id
            FROM household_entities
            WHERE workspace_id = %s
                AND entity_type = %s
                AND lower(display_name) = lower(%s)
            LIMIT 1
            """,
            (workspace_id, payload["entity_type"], payload["display_name"]),
        )
        existing = cursor.fetchone()
        if existing is None:
            cursor.execute(
                """
                INSERT INTO household_entities (
                    workspace_id,
                    entity_type,
                    display_name,
                    metadata
                )
                VALUES (%s, %s, %s, %s)
                RETURNING id, entity_type, display_name, metadata
                """,
                (
                    workspace_id,
                    payload["entity_type"],
                    payload["display_name"],
                    Jsonb(payload.get("metadata") or {}),
                ),
            )
            entity = cursor.fetchone()
            if entity is None:
                raise RuntimeError("Suggestion entity create did not return an entity.")
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="household_entity.created",
                entity_type=entity["entity_type"],
                entity_id=entity["id"],
                metadata={
                    "display_name": entity["display_name"],
                    "created_from_graph_suggestion_id": str(row["id"]),
                },
            )
            entity_id = entity["id"]
        else:
            entity_id = existing["id"]

        relationship = payload.get("relationship")
        if isinstance(relationship, dict):
            _create_relationship_from_payload(
                cursor,
                workspace_id=workspace_id,
                payload={
                    **relationship,
                    "source_entity_id": str(entity_id),
                },
            )
        return

    if action_type == "update_metadata":
        entity_id = payload["entity_id"]
        cursor.execute(
            """
            UPDATE household_entities
            SET metadata = metadata || %s::jsonb, updated_at = now()
            WHERE workspace_id = %s
                AND id = %s
            RETURNING id, entity_type, display_name, metadata
            """,
            (Jsonb(payload.get("metadata") or {}), workspace_id, entity_id),
        )
        entity = cursor.fetchone()
        if entity:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="household_entity.updated",
                entity_type=entity["entity_type"],
                entity_id=entity["id"],
                metadata={
                    "updated_fields": ["metadata"],
                    "updated_from_graph_suggestion_id": str(row["id"]),
                },
            )
        return

    if action_type == "merge_duplicate_entities":
        return

    raise ValueError(f"Unsupported graph suggestion action: {action_type}")


def _create_relationship_from_payload(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    payload: dict[str, object],
) -> None:
    source_type = str(payload["source_entity_type"])
    source_id = str(payload["source_entity_id"])
    relationship_type = _normalize_kind(str(payload["relationship_type"]))
    target_type = str(payload["target_entity_type"])
    target_id = str(payload["target_entity_id"])

    cursor.execute(
        """
        SELECT id
        FROM entity_relationships
        WHERE workspace_id = %s
            AND source_entity_type = %s
            AND source_entity_id = %s
            AND relationship_type = %s
            AND target_entity_type = %s
            AND target_entity_id = %s
        LIMIT 1
        """,
        (workspace_id, source_type, source_id, relationship_type, target_type, target_id),
    )
    if cursor.fetchone() is not None:
        return

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
        VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
        RETURNING
            id,
            source_entity_type,
            source_entity_id,
            relationship_type,
            target_entity_type,
            target_entity_id,
            provenance_document_id,
            confidence
        """,
        (
            workspace_id,
            source_type,
            source_id,
            relationship_type,
            target_type,
            target_id,
            payload.get("provenance_document_id"),
            payload.get("confidence"),
        ),
    )
    relationship = cursor.fetchone()
    if relationship is None:
        raise RuntimeError("Suggestion relationship create did not return a relationship.")
    record_audit_event(
        cursor,
        workspace_id=workspace_id,
        event_type="relationship.created",
        entity_type="relationship",
        entity_id=relationship["id"],
        metadata={
            "source_entity_type": relationship["source_entity_type"],
            "source_entity_id": str(relationship["source_entity_id"]),
            "relationship_type": relationship["relationship_type"],
            "target_entity_type": relationship["target_entity_type"],
            "target_entity_id": str(relationship["target_entity_id"]),
            "provenance_document_id": (
                str(relationship["provenance_document_id"])
                if relationship["provenance_document_id"]
                else None
            ),
            "confidence": (
                float(relationship["confidence"])
                if relationship["confidence"] is not None
                else None
            ),
        },
    )


def _get_pending_suggestion(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
    suggestion_id: str,
) -> dict[str, Any]:
    cursor.execute(
        """
        SELECT
            id,
            action_type,
            action_payload,
            confidence,
            reasoning,
            affected_entities,
            status,
            source_document_id,
            source_bill_id
        FROM graph_suggestions
        WHERE workspace_id = %s
            AND id = %s
            AND status = 'pending'
        """,
        (workspace_id, suggestion_id),
    )
    row = cursor.fetchone()
    if row is None:
        raise LookupError("Graph suggestion not found.")
    return row


def _load_entities(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[dict[str, Any]]:
    cursor.execute(
        """
        SELECT id, entity_type, display_name, metadata
        FROM household_entities
        WHERE workspace_id = %s
        ORDER BY created_at
        """,
        (workspace_id,),
    )
    return cursor.fetchall()


def _load_relationship_keys(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> set[tuple[str, str, str, str, str]]:
    cursor.execute(
        """
        SELECT
            source_entity_type,
            source_entity_id,
            relationship_type,
            target_entity_type,
            target_entity_id
        FROM entity_relationships
        WHERE workspace_id = %s
        """,
        (workspace_id,),
    )
    return {
        (
            row["source_entity_type"],
            str(row["source_entity_id"]),
            row["relationship_type"],
            row["target_entity_type"],
            str(row["target_entity_id"]),
        )
        for row in cursor.fetchall()
    }


def _load_documents(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> dict[str, dict[str, Any]]:
    cursor.execute(
        """
        SELECT
            d.id,
            d.original_filename,
            d.suggested_cabinet_path,
            d.confirmed_cabinet_path,
            COALESCE(t.text_content, '') AS text_content
        FROM documents d
        LEFT JOIN document_texts t ON t.document_id = d.id
        WHERE d.workspace_id = %s
        """,
        (workspace_id,),
    )
    return {str(row["id"]): row for row in cursor.fetchall()}


def _load_bills(
    cursor: Cursor[dict[str, Any]],
    *,
    workspace_id: object,
) -> list[dict[str, Any]]:
    cursor.execute(
        """
        SELECT
            id,
            document_id,
            supplier_entity_id,
            supplier,
            invoice_number,
            category,
            classification,
            extraction_confidence
        FROM bills
        WHERE workspace_id = %s
        ORDER BY created_at DESC
        """,
        (workspace_id,),
    )
    return cursor.fetchall()


def _suggestion_from_row(row: dict[str, Any]) -> GraphSuggestion:
    return GraphSuggestion(
        id=str(row["id"]),
        confidence=float(row["confidence"]),
        suggested_action=GraphSuggestionAction(row["action_type"]),
        reasoning=row["reasoning"],
        affected_entities=list(row["affected_entities"] or []),
        status=GraphSuggestionStatus(row["status"]),
        action_payload=dict(row["action_payload"] or {}),
        source_document_id=str(row["source_document_id"]) if row["source_document_id"] else None,
        source_bill_id=str(row["source_bill_id"]) if row["source_bill_id"] else None,
    )


def _match_property_for_document(
    document: dict[str, Any] | None,
    entities: list[dict[str, Any]],
) -> dict[str, Any] | None:
    properties = [entity for entity in entities if entity["entity_type"] == "property"]
    if not properties:
        return None

    for entity in properties:
        if _document_mentions_entity(document, entity):
            return entity

    if len(properties) == 1:
        return properties[0]
    return None


def _document_mentions_entity(
    document: dict[str, Any] | None,
    entity: dict[str, Any],
) -> bool:
    if not document:
        return False
    haystack = " ".join(
        str(value or "")
        for value in (
            document.get("original_filename"),
            document.get("suggested_cabinet_path"),
            document.get("confirmed_cabinet_path"),
            document.get("text_content"),
        )
    ).lower()
    needles = [str(entity["display_name"])]
    metadata = entity.get("metadata") or {}
    if isinstance(metadata, dict):
        needles.extend(str(value) for value in metadata.values() if isinstance(value, str))
    return any(needle and needle.lower() in haystack for needle in needles)


def _entity_exists(
    entities: list[dict[str, Any]],
    *,
    entity_type: str,
    display_name: str,
) -> bool:
    return any(
        entity["entity_type"] == entity_type
        and str(entity["display_name"]).lower() == display_name.lower()
        for entity in entities
    )


def _relationship_exists(
    relationships: set[tuple[str, str, str, str, str]],
    *,
    source_type: str,
    source_id: str,
    relationship_type: str,
    target_type: str,
    target_id: str,
) -> bool:
    return (
        source_type,
        source_id,
        relationship_type,
        target_type,
        target_id,
    ) in relationships


def _utility_account_name(bill: dict[str, Any]) -> str | None:
    values = [
        str(bill.get("category") or ""),
        str(bill.get("classification") or ""),
        str(bill.get("supplier") or ""),
    ]
    joined = " ".join(values).lower()
    for token, account_name in UTILITY_CATEGORIES.items():
        if token in joined:
            return account_name
    return None


def _confidence(bill: dict[str, Any], default: float) -> float:
    confidence = bill.get("extraction_confidence")
    if isinstance(confidence, (float, int)):
        return min(0.99, max(0.1, float(confidence)))
    return default


def _affected(entity_type: str, entity_id: str, display_name: str) -> dict[str, object]:
    return {
        "entity_type": entity_type,
        "entity_id": entity_id,
        "display_name": display_name,
    }


def _bill_name(bill: dict[str, Any]) -> str:
    invoice_number = bill.get("invoice_number")
    return f"{bill['supplier']} {invoice_number}" if invoice_number else str(bill["supplier"])


def _document_name(document: dict[str, Any] | None, fallback_id: str) -> str:
    return str(document["original_filename"]) if document else fallback_id


def _normalize_kind(value: str) -> str:
    return value.strip().lower().replace(" ", "_")
