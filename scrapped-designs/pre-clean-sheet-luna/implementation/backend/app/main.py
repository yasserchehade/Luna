from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from app.api.audit import router as audit_router
from app.api.bills import router as bills_router
from app.api.documents import router as documents_router
from app.api.household import router as household_router
from app.api.knowledge import router as knowledge_router
from app.api.work import router as work_router
from app.core.config import settings

app = FastAPI(title=settings.app_name)

app.add_middleware(
    CORSMiddleware,
    allow_origins=[origin.strip() for origin in settings.cors_origins.split(",") if origin.strip()],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": settings.app_name}


app.include_router(bills_router, prefix="/api")
app.include_router(documents_router, prefix="/api")
app.include_router(household_router, prefix="/api")
app.include_router(knowledge_router, prefix="/api")
app.include_router(audit_router, prefix="/api")
app.include_router(work_router, prefix="/api")
