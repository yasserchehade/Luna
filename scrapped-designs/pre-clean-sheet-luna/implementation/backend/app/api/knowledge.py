from fastapi import APIRouter

from app.db import get_connection, get_default_workspace_id
from app.models.knowledge import KnowledgeAskRequest, KnowledgeAskResponse
from app.services.audit import record_audit_event
from app.services.knowledge import answer_household_question

router = APIRouter(prefix="/knowledge", tags=["knowledge"])


@router.post("/ask", response_model=KnowledgeAskResponse)
def ask_knowledge(request: KnowledgeAskRequest) -> KnowledgeAskResponse:
    response = answer_household_question(request.question)
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="knowledge.question_answered",
                entity_type="knowledge",
                metadata={
                    "question": request.question,
                    "confidence": response.confidence,
                    "source_count": len(response.sources),
                },
            )
    return response
