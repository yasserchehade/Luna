from typing import Annotated

from fastapi import APIRouter, File, HTTPException, UploadFile, status
from psycopg.types.json import Jsonb

from app.db import get_connection, get_default_workspace_id
from app.models.document import Document, DocumentText
from app.services.document_text import extract_pdf_text
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

            extracted_text = extract_pdf_text(stored.storage_path)
            cursor.execute(
                """
                INSERT INTO document_texts (
                    document_id,
                    text_content,
                    extraction_method,
                    page_count,
                    character_count,
                    metadata
                )
                VALUES (%s, %s, %s, %s, %s, %s)
                """,
                (
                    stored.document_id,
                    extracted_text.text_content,
                    extracted_text.extraction_method,
                    extracted_text.page_count,
                    extracted_text.character_count,
                    Jsonb(extracted_text.metadata),
                ),
            )

    return Document(
        id=str(row["id"]),
        original_filename=row["original_filename"],
        content_type=row["content_type"],
        sha256=row["sha256"],
        received_at=row["received_at"],
        text_extracted=True,
        page_count=extracted_text.page_count,
        character_count=extracted_text.character_count,
    )


@router.get("/{document_id}", response_model=Document)
def get_document(document_id: str) -> Document:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    d.id,
                    d.original_filename,
                    d.content_type,
                    d.sha256,
                    d.received_at,
                    t.page_count,
                    t.character_count
                FROM documents d
                LEFT JOIN document_texts t ON t.document_id = d.id
                WHERE d.id = %s
                """,
                (document_id,),
            )
            row = cursor.fetchone()

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Document not found.",
        )

    return Document(
        id=str(row["id"]),
        original_filename=row["original_filename"],
        content_type=row["content_type"],
        sha256=row["sha256"],
        received_at=row["received_at"],
        text_extracted=row["character_count"] is not None,
        page_count=row["page_count"],
        character_count=row["character_count"],
    )


@router.get("/{document_id}/text", response_model=DocumentText)
def get_document_text(document_id: str) -> DocumentText:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    document_id,
                    text_content,
                    extraction_method,
                    page_count,
                    character_count,
                    extracted_at
                FROM document_texts
                WHERE document_id = %s
                """,
                (document_id,),
            )
            row = cursor.fetchone()

    if row is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Document text not found.",
        )

    return DocumentText(
        document_id=str(row["document_id"]),
        text_content=row["text_content"],
        extraction_method=row["extraction_method"],
        page_count=row["page_count"],
        character_count=row["character_count"],
        extracted_at=row["extracted_at"],
    )
