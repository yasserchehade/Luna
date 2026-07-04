from datetime import datetime

from pydantic import BaseModel, Field


class AuditEvent(BaseModel):
    id: str
    event_type: str
    entity_type: str | None = None
    entity_id: str | None = None
    metadata: dict[str, object] = Field(default_factory=dict)
    created_at: datetime
