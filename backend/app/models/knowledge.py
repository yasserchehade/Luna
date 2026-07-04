from pydantic import BaseModel, Field


class KnowledgeAskRequest(BaseModel):
    question: str = Field(min_length=2, max_length=500)


class KnowledgeSource(BaseModel):
    source_type: str
    source_id: str
    title: str
    detail: str | None = None


class KnowledgeAskResponse(BaseModel):
    question: str
    answer: str
    confidence: float
    sources: list[KnowledgeSource]
    suggested_next_actions: list[str] = Field(default_factory=list)
