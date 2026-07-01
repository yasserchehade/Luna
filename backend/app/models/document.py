from datetime import datetime

from pydantic import BaseModel


class Document(BaseModel):
    id: str
    original_filename: str
    content_type: str
    sha256: str
    received_at: datetime
