from celery import Celery

from app.core.config import settings

celery_app = Celery(
    "luna",
    broker=settings.redis_url,
    backend=settings.redis_url,
)


@celery_app.task(name="luna.healthcheck")
def healthcheck() -> str:
    return "ok"
