from psycopg import Cursor
from psycopg.types.json import Jsonb


def record_supplier_template_match(
    cursor: Cursor,
    *,
    workspace_id,
    supplier_entity_id,
    document_id: str,
    extraction: dict[str, object],
) -> None:
    profile = extraction.get("supplier_profile")
    if not isinstance(profile, dict):
        return

    profile_key = str(profile.get("profile_key") or "unknown_supplier")
    supplier_name = str(profile.get("supplier_name") or extraction.get("supplier") or "Unknown supplier")
    aliases = profile.get("aliases") if isinstance(profile.get("aliases"), list) else []
    category = profile.get("category")
    matched_anchors = profile.get("matched_anchors") if isinstance(profile.get("matched_anchors"), list) else []
    missing_anchors = profile.get("missing_anchors") if isinstance(profile.get("missing_anchors"), list) else []
    expected_anchors = profile.get("expected_anchors") if isinstance(profile.get("expected_anchors"), list) else []
    fingerprint = str(profile.get("template_fingerprint") or "")
    template_status = str(profile.get("template_status") or "unknown")
    profile_status = "needs_review" if template_status in {"changed", "needs_review"} else "active"

    cursor.execute(
        """
        INSERT INTO supplier_profiles (
            workspace_id,
            supplier_entity_id,
            profile_key,
            supplier_name,
            aliases,
            category,
            status
        )
        VALUES (%s, %s, %s, %s, %s, %s, %s)
        ON CONFLICT (workspace_id, profile_key)
        DO UPDATE SET
            supplier_entity_id = COALESCE(EXCLUDED.supplier_entity_id, supplier_profiles.supplier_entity_id),
            supplier_name = EXCLUDED.supplier_name,
            aliases = EXCLUDED.aliases,
            category = EXCLUDED.category,
            status = CASE
                WHEN supplier_profiles.status = 'archived' THEN supplier_profiles.status
                ELSE EXCLUDED.status
            END,
            updated_at = now()
        RETURNING id
        """,
        (
            workspace_id,
            supplier_entity_id,
            profile_key,
            supplier_name,
            Jsonb(aliases),
            category,
            profile_status,
        ),
    )
    supplier_profile = cursor.fetchone()
    if supplier_profile is None:
        raise RuntimeError("Supplier profile upsert did not return an id.")

    cursor.execute(
        """
        INSERT INTO supplier_template_versions (
            supplier_profile_id,
            version_label,
            fingerprint,
            expected_anchors,
            status,
            first_seen_document_id
        )
        VALUES (%s, %s, %s, %s, %s, %s)
        ON CONFLICT (supplier_profile_id, fingerprint)
        DO UPDATE SET
            status = CASE
                WHEN supplier_template_versions.status = 'archived'
                    THEN supplier_template_versions.status
                ELSE EXCLUDED.status
            END,
            updated_at = now()
        RETURNING id
        """,
        (
            supplier_profile["id"],
            str(profile.get("version_label") or "observed"),
            fingerprint,
            Jsonb(expected_anchors),
            "needs_review" if template_status in {"changed", "needs_review"} else "active",
            document_id,
        ),
    )
    template_version = cursor.fetchone()
    if template_version is None:
        raise RuntimeError("Supplier template version upsert did not return an id.")

    cursor.execute(
        """
        INSERT INTO document_template_matches (
            document_id,
            supplier_profile_id,
            template_version_id,
            fingerprint,
            matched_anchors,
            missing_anchors,
            confidence,
            status
        )
        VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
        """,
        (
            document_id,
            supplier_profile["id"],
            template_version["id"],
            fingerprint,
            Jsonb(matched_anchors),
            Jsonb(missing_anchors),
            extraction.get("confidence"),
            template_status if template_status in {"known", "unknown", "changed", "needs_review"} else "unknown",
        ),
    )
