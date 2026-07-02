from datetime import datetime

from pydantic import BaseModel


class Document(BaseModel):
    id: str
    original_filename: str
    content_type: str
    sha256: str
    storage_provider: str = "local_folder"
    storage_path: str
    cabinet_status: str = "unplanned"
    suggested_cabinet_path: str | None = None
    confirmed_cabinet_path: str | None = None
    received_at: datetime
    text_extracted: bool = False
    page_count: int | None = None
    character_count: int | None = None


class DocumentCabinetPlan(BaseModel):
    document_id: str
    storage_provider: str
    cabinet_status: str
    suggested_cabinet_path: str
    reasons: list[str]


class DocumentText(BaseModel):
    document_id: str
    text_content: str
    extraction_method: str
    page_count: int | None = None
    character_count: int
    extracted_at: datetime
