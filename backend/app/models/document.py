from datetime import datetime

from pydantic import BaseModel


class Document(BaseModel):
    id: str
    original_filename: str
    content_type: str
    sha256: str
    received_at: datetime
    text_extracted: bool = False
    page_count: int | None = None
    character_count: int | None = None


class DocumentText(BaseModel):
    document_id: str
    text_content: str
    extraction_method: str
    page_count: int | None = None
    character_count: int
    extracted_at: datetime
