from typing import Annotated

from fastapi import APIRouter, File, HTTPException, UploadFile, status

from app.db import get_connection, get_default_workspace_id
from app.models.document import Document
from app.storage.documents import store_uploaded_document

router = APIRouter(prefix="/documents", tags=["documents"])


@router.post("", response_model=Document, status_code=status.HTTP_201_CREATED)
async def upload_document(file: Annotated[UploadFile, File(...)]) -> Document:
    if file.content_type not in {"application/pdf", "application/x-pdf"}:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Only PDF bill or invoice uploads are supported.",
        )

    try:
        stored = await store_uploaded_document(file)
    except ValueError as error:
        raise HTTPException(
            status_code=status.HTTP_413_REQUEST_ENTITY_TOO_LARGE,
            detail=str(error),
        ) from error

    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            cursor.execute(
                """
                INSERT INTO documents (
                    id,
                    workspace_id,
                    source,
                    original_filename,
                    content_type,
                    storage_path,
                    sha256
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                RETURNING id, original_filename, content_type, sha256, received_at
                """,
                (
                    stored.document_id,
                    workspace_id,
                    "upload",
                    stored.original_filename,
                    stored.content_type,
                    stored.storage_path,
                    stored.sha256,
                ),
            )
            row = cursor.fetchone()

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Document upload could not be saved.",
        )

    return Document(
        id=str(row["id"]),
        original_filename=row["original_filename"],
        content_type=row["content_type"],
        sha256=row["sha256"],
        received_at=row["received_at"],
    )
