from typing import Protocol


class BillExtractor(Protocol):
    def extract_from_document(self, document_id: str) -> dict[str, object]:
        """Extract bill fields from a stored document."""


class StubBillExtractor:
    def extract_from_document(self, document_id: str) -> dict[str, object]:
        return {
            "supplier": "Unknown supplier",
            "amount": None,
            "due_date": None,
            "invoice_number": None,
            "category": None,
            "classification": None,
            "confidence": 0.0,
            "notes": f"Stub extraction for document {document_id}.",
        }


def get_extractor() -> BillExtractor:
    return StubBillExtractor()
