from fastapi import APIRouter

from app.models.knowledge import KnowledgeAskRequest, KnowledgeAskResponse
from app.services.knowledge import answer_household_question

router = APIRouter(prefix="/knowledge", tags=["knowledge"])


@router.post("/ask", response_model=KnowledgeAskResponse)
def ask_knowledge(request: KnowledgeAskRequest) -> KnowledgeAskResponse:
    return answer_household_question(request.question)
