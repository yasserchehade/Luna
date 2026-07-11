import re
from hashlib import sha256
from datetime import date, datetime, timedelta
from typing import Protocol

from app.db import get_connection


class BillExtractor(Protocol):
    def extract_from_document(self, document_id: str) -> dict[str, object]:
        """Extract bill fields from a stored document."""


MONTHS = {
    "jan": 1,
    "january": 1,
    "feb": 2,
    "february": 2,
    "mar": 3,
    "march": 3,
    "apr": 4,
    "april": 4,
    "may": 5,
    "jun": 6,
    "june": 6,
    "jul": 7,
    "july": 7,
    "aug": 8,
    "august": 8,
    "sep": 9,
    "sept": 9,
    "september": 9,
    "oct": 10,
    "october": 10,
    "nov": 11,
    "november": 11,
    "dec": 12,
    "december": 12,
}

SUPPLIER_PROFILES = {
    "agl": {
        "supplier_name": "AGL Sales Pty Ltd",
        "aliases": ["AGL", "AGL Sales Pty Ltd", "agl.com.au"],
        "category": "utilities",
        "expected_anchors": ["Amount due", "Due date", "Account number", "Tax Invoice"],
    },
    "tango_energy": {
        "supplier_name": "Tango Energy",
        "aliases": ["Tango Energy", "Pacific Blue Retail Pty Ltd", "tangoenergy.com"],
        "category": "utilities",
        "expected_anchors": ["Tax invoice number", "Due date", "Total amount due", "NATIONAL METER IDENTIFIER"],
    },
    "water_corporation": {
        "supplier_name": "Water Corporation",
        "aliases": ["Water Corporation", "watercorporation.com.au"],
        "category": "utilities",
        "expected_anchors": ["PLEASE PAY", "DUE BY", "ACCOUNT NUMBER", "WATER USE PERIOD"],
    },
    "yarra_valley_water": {
        "supplier_name": "Yarra Valley Water",
        "aliases": ["Yarra Valley Water", "YVW", "Yarra Valley Water ABN"],
        "category": "utilities",
        "expected_anchors": ["Amount due", "Due date", "Total balance", "Tax Invoice"],
    },
    "imc_insurance_brokers": {
        "supplier_name": "IMC Insurance Brokers",
        "aliases": ["IMC Insurance Brokers", "AFSL: 229344"],
        "category": "insurance",
        "expected_anchors": ["NEW BUSINESS TAX INVOICE", "Policy Number", "Invoice No", "Total Due"],
    },
}


class TextBillExtractor:
    def extract_from_document(self, document_id: str) -> dict[str, object]:
        document = _load_document_text(document_id)
        if document is None:
            return _empty_extraction(document_id, "No extracted document text found.")

        text = document["text_content"] or ""
        supplier = _extract_supplier(text, document["original_filename"])
        amount = _extract_amount(text)
        due_date = _extract_due_date(text)
        invoice_number = _extract_invoice_number(text)
        category = _classify_category(text, supplier)
        profile = _build_supplier_profile_match(text, supplier)
        confidence = _confidence(supplier, amount, due_date, invoice_number)

        return {
            "provider": "local_rules",
            "supplier": supplier,
            "amount": amount,
            "due_date": due_date,
            "invoice_number": invoice_number,
            "category": category,
            "classification": "property" if category in {"utilities", "insurance"} else None,
            "confidence": confidence,
            "supplier_profile": profile,
            "notes": f"Local text extraction for document {document_id}.",
        }


def _load_document_text(document_id: str) -> dict[str, object] | None:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT d.original_filename, t.text_content
                FROM documents d
                LEFT JOIN document_texts t ON t.document_id = d.id
                WHERE d.id = %s
                """,
                (document_id,),
            )
            return cursor.fetchone()


def _empty_extraction(document_id: str, note: str) -> dict[str, object]:
    return {
        "provider": "local_rules",
        "supplier": "Unknown supplier",
        "amount": None,
        "due_date": None,
        "invoice_number": None,
        "category": None,
        "classification": None,
        "confidence": 0.0,
        "notes": f"{note} Document {document_id}.",
    }


def _extract_supplier(text: str, filename: str) -> str:
    lowered = text.lower()
    known_suppliers = {
        "agl sales": "AGL Sales Pty Ltd",
        "agl energy": "AGL Energy",
        "pacific blue retail": "Tango Energy",
        "tango energy": "Tango Energy",
        "watercorporation": "Water Corporation",
        "water corporation": "Water Corporation",
        "yarra valley water": "Yarra Valley Water",
        "imc insurance brokers": "IMC Insurance Brokers",
        "allianz australia insurance": "Allianz Australia Insurance Limited",
    }
    for needle, supplier in known_suppliers.items():
        if needle in lowered:
            return supplier

    for line in _non_empty_lines(text):
        clean = line.strip(" :-")
        if len(clean) > 3 and not re.search(r"\$|\d{2}/\d{2}/\d{4}", clean):
            return clean[:120]

    return filename.rsplit(".", 1)[0] or "Unknown supplier"


def _build_supplier_profile_match(text: str, supplier: str) -> dict[str, object]:
    profile_key = _profile_key_for_supplier(supplier)
    profile = SUPPLIER_PROFILES.get(profile_key)

    if profile is None:
        first_lines = _non_empty_lines(text)[:12]
        return {
            "profile_key": _normalize_key(supplier),
            "supplier_name": supplier,
            "aliases": [supplier] if supplier != "Unknown supplier" else [],
            "category": None,
            "expected_anchors": [],
            "matched_anchors": [],
            "missing_anchors": [],
            "template_fingerprint": _fingerprint(first_lines),
            "template_status": "unknown",
            "version_label": "observed",
        }

    expected_anchors = list(profile["expected_anchors"])
    matched_anchors = [anchor for anchor in expected_anchors if anchor.lower() in text.lower()]
    missing_anchors = [anchor for anchor in expected_anchors if anchor not in matched_anchors]
    template_status = "known" if not missing_anchors else "changed"

    return {
        "profile_key": profile_key,
        "supplier_name": profile["supplier_name"],
        "aliases": profile["aliases"],
        "category": profile["category"],
        "expected_anchors": expected_anchors,
        "matched_anchors": matched_anchors,
        "missing_anchors": missing_anchors,
        "template_fingerprint": _fingerprint(_fingerprint_lines(text, expected_anchors)),
        "template_status": template_status,
        "version_label": "observed",
    }


def _profile_key_for_supplier(supplier: str) -> str:
    normalized = _normalize_key(supplier)
    if normalized.startswith("agl"):
        return "agl"
    if "tango_energy" in normalized:
        return "tango_energy"
    if "water_corporation" in normalized:
        return "water_corporation"
    if "yarra_valley_water" in normalized:
        return "yarra_valley_water"
    if "imc_insurance_brokers" in normalized:
        return "imc_insurance_brokers"
    return normalized


def _normalize_key(value: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "_", value.lower()).strip("_")
    return normalized or "unknown_supplier"


def _fingerprint_lines(text: str, anchors: list[str]) -> list[str]:
    lines = _non_empty_lines(text)
    selected: list[str] = []
    for anchor in anchors:
        for index, line in enumerate(lines):
            if anchor.lower() in line.lower():
                selected.extend(lines[index : index + 3])
                break
    return selected or lines[:12]


def _fingerprint(lines: list[str]) -> str:
    normalized = "\n".join(re.sub(r"\d", "0", line.lower()).strip() for line in lines)
    return sha256(normalized.encode("utf-8")).hexdigest()


def _extract_amount(text: str) -> float | None:
    patterns = [
        r"Amount\s+due\s*\n?\s*\$?\s*([\d,]+\.\d{2})",
        r"Total\s+Due:\s*\$?\s*([\d,]+\.\d{2})",
        r"Total\s+balance\s*\$?\s*([\d,]+\.\d{2})",
        r"Total\s+amount\s+due\s*\$?\s*([\d,]+\.\d{2})",
        r"PLEASE\s+PAY:\s*\n?\s*\$?\s*([\d,]+\.\d{2})",
        r"Invoice\s+Total\s*\n?.*?\$?\s*([\d,]+\.\d{2})",
        r"Total\s+this\s+bill.*?\$?\s*([\d,]+\.\d{2})",
        r"Tax\s+Invoice\s*\n?\s*\$?\s*([\d,]+\.\d{2})",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE | re.DOTALL)
        if match:
            return _money_to_float(match.group(1))
    return None


def _money_to_float(value: str) -> float:
    return float(value.replace(",", ""))


def _extract_due_date(text: str) -> str | None:
    direct_patterns = [
        r"Due\s+date\s*\n?\s*([0-9]{1,2}\s+[A-Za-z]{3,9}\s+[0-9]{4})",
        r"Due\s+date\s*\n?\s*([0-9]{1,2}/[0-9]{1,2}/[0-9]{4})",
        r"Due\s+by:\s*\n?\s*([0-9]{1,2}\s+[A-Za-z]{3,9}\s+[0-9]{4})",
        r"Due\s+by:\s*\n?\s*([0-9]{1,2}/[0-9]{1,2}/[0-9]{4})",
        r"Tax\s+Invoice\s*\n?\s*\$?[\d,]+\.\d{2}\s*\n?\s*([0-9]{1,2}\s+[A-Za-z]{3,9}\s+[0-9]{4})",
    ]
    for pattern in direct_patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE)
        if match and (parsed := _parse_date(match.group(1))):
            return parsed.isoformat()

    relative = re.search(
        r"payment\s+is\s+required\s+within\s+(\d+)\s+days\s+from\s+([0-9]{1,2}/[0-9]{1,2}/[0-9]{4})",
        text,
        flags=re.IGNORECASE,
    )
    if relative and (start := _parse_date(relative.group(2))):
        return (start + timedelta(days=int(relative.group(1)))).isoformat()

    return None


def _parse_date(value: str) -> date | None:
    value = " ".join(value.strip().replace(",", "").split())
    for fmt in ("%d/%m/%Y", "%d %b %Y", "%d %B %Y"):
        try:
            return datetime.strptime(value.title(), fmt).date()
        except ValueError:
            pass

    parts = value.split()
    if len(parts) == 3 and parts[1].lower() in MONTHS:
        return date(int(parts[2]), MONTHS[parts[1].lower()], int(parts[0]))
    return None


def _extract_invoice_number(text: str) -> str | None:
    patterns = [
        r"Invoice\s+No:\s*([A-Za-z0-9-]+)",
        r"Invoice\s+Number:\s*([A-Za-z0-9-]+)",
        r"Invoice\s+number\s*\n\s*([A-Za-z0-9-]+)",
        r"Tax\s+invoice\s+number\s*([A-Za-z0-9-]+)",
        r"BILL\s+ID\s*([A-Za-z0-9-]+)",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE)
        if match and _looks_like_identifier(match.group(1)):
            return match.group(1).strip()
    return None


def _looks_like_identifier(value: str) -> bool:
    value = value.strip()
    if value.lower() in {"issue", "date", "account", "number"}:
        return False
    return any(character.isdigit() for character in value)


def _classify_category(text: str, supplier: str) -> str | None:
    haystack = f"{supplier}\n{text}".lower()
    if any(term in haystack for term in ("insurance", "insured", "policy", "premium")):
        return "insurance"
    if any(
        term in haystack
        for term in ("agl", "electricity", "energy", "water", "sewerage", "utility")
    ):
        return "utilities"
    return None


def _confidence(
    supplier: str,
    amount: float | None,
    due_date: str | None,
    invoice_number: str | None,
) -> float:
    score = 0.15
    if supplier != "Unknown supplier":
        score += 0.25
    if amount is not None:
        score += 0.25
    if due_date is not None:
        score += 0.25
    if invoice_number is not None:
        score += 0.10
    return min(score, 1.0)


def _non_empty_lines(text: str) -> list[str]:
    return [line.strip() for line in text.splitlines() if line.strip()]


def get_extractor() -> BillExtractor:
    return TextBillExtractor()
