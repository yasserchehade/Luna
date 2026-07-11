from celery import Celery

from app.core.config import settings
from app.db import get_connection, get_default_workspace_id
from app.services.obligations import (
    backfill_confirmed_bill_obligations,
    run_obligation_maintenance,
)

celery_app = Celery(
    "luna",
    broker=settings.redis_url,
    backend=settings.redis_url,
)


@celery_app.task(name="luna.healthcheck")
def healthcheck() -> str:
    return "ok"


@celery_app.task(name="luna.obligations.maintenance")
def obligation_maintenance() -> dict[str, int]:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            return run_obligation_maintenance(cursor, workspace_id=workspace_id)


@celery_app.task(name="luna.obligations.backfill_confirmed_bills")
def backfill_confirmed_bill_obligations_task() -> dict[str, int]:
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            obligations = backfill_confirmed_bill_obligations(
                cursor,
                workspace_id=workspace_id,
            )
            return {"backfilled_obligations": len(obligations)}
