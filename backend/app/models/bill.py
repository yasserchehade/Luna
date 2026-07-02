from enum import StrEnum

from pydantic import BaseModel, Field


class BillStatus(StrEnum):
    draft = "draft"
    unpaid = "unpaid"
    paid = "paid"
    overdue = "overdue"
    archived = "archived"


class Bill(BaseModel):
    id: str
    supplier: str
    supplier_entity_id: str | None = None
    amount: float | None = None
    due_date: str | None = None
    invoice_number: str | None = None
    category: str | None = None
    classification: str | None = None
    status: BillStatus
    document_id: str | None = None
    currency: str = "AUD"


class BillIngestRequest(BaseModel):
    document_id: str


class BillIngestResponse(BaseModel):
    document_id: str
    bill: Bill
    extraction: dict[str, object]


class BillUpdate(BaseModel):
    supplier: str | None = Field(default=None, min_length=1)
    amount: float | None = None
    due_date: str | None = None
    invoice_number: str | None = None
    category: str | None = None
    classification: str | None = None


class BillActionResponse(BaseModel):
    bill: Bill
