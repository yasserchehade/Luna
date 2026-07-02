from typing import Annotated

from fastapi import APIRouter, File, HTTPException, UploadFile, status
from psycopg.types.json import Jsonb

from app.db import get_connection, get_default_workspace_id
from app.models.document import (
    Document,
    DocumentCabinetPlan,
    DocumentCabinetConfirmRequest,
    DocumentCabinetConfirmResponse,
    DocumentText,
)
from app.services.cabinet import confirm_document_cabinet_path, save_document_cabinet_plan
from app.services.document_text import extract_pdf_text
from app.storage.documents import store_uploaded_document

router = APIRouter(prefix="/documents", tags=["documents"])


def _document_from_row(row, text_extracted: bool | None = None) -> Document:
    character_count = row["character_count"] if "character_count" in row else None
    return Document(
        id=str(row["id"]),
        original_filename=row["original_filename"],
        content_type=row["content_type"],
        sha256=row["sha256"],
        storage_provider=row["storage_provider"],
        storage_path=row["storage_path"],
        cabinet_status=row["cabinet_status"],
        suggested_cabinet_path=row["suggested_cabinet_path"],
        confirmed_cabinet_path=row["confirmed_cabinet_path"],
        received_at=row["received_at"],
        text_extracted=text_extracted if text_extracted is not None else character_count is not None,
        page_count=row["page_count"] if "page_count" in row else None,
        character_count=character_count,
    )


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
                    storage_provider,
                    storage_path,
                    sha256
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING
                    id,
                    original_filename,
                    content_type,
                    storage_provider,
                    storage_path,
                    cabinet_status,
                    suggested_cabinet_path,
                    confirmed_cabinet_path,
                    sha256,
                    received_at
                """,
                (
                    stored.document_id,
                    workspace_id,
                    "upload",
                    stored.original_filename,
                    stored.content_type,
                    "local_folder",
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

    return _document_from_row(
        {
            **row,
            "page_count": extracted_text.page_count,
            "character_count": extracted_text.character_count,
        },
        text_extracted=True,
    )


@router.get("", response_model=list[Document])
def list_documents() -> list[Document]:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    d.id,
                    d.original_filename,
                    d.content_type,
                    d.storage_provider,
                    d.storage_path,
                    d.cabinet_status,
                    d.suggested_cabinet_path,
                    d.confirmed_cabinet_path,
                    d.sha256,
                    d.received_at,
                    t.page_count,
                    t.character_count
                FROM documents d
                LEFT JOIN document_texts t ON t.document_id = d.id
                ORDER BY d.received_at DESC
                LIMIT 20
                """
            )
            rows = cursor.fetchall()

    return [_document_from_row(row) for row in rows]


@router.get("/{document_id}", response_model=Document)
def get_document(document_id: str) -> Document:
    document = _get_document_or_404(document_id)
    return document


def _get_document_or_404(document_id: str) -> Document:
    with get_connection() as connection:
        with connection.cursor() as cursor:
            cursor.execute(
                """
                SELECT
                    d.id,
                    d.original_filename,
                    d.content_type,
                    d.storage_provider,
                    d.storage_path,
                    d.cabinet_status,
                    d.suggested_cabinet_path,
                    d.confirmed_cabinet_path,
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

    return _document_from_row(row)


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


@router.post("/{document_id}/cabinet-plan", response_model=DocumentCabinetPlan)
def plan_document_cabinet(document_id: str) -> DocumentCabinetPlan:
    try:
        plan = save_document_cabinet_plan(document_id)
    except ValueError as error:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=str(error),
        ) from error

    return DocumentCabinetPlan(
        document_id=str(plan["document_id"]),
        storage_provider=str(plan["storage_provider"]),
        cabinet_status=str(plan["cabinet_status"]),
        suggested_cabinet_path=str(plan["suggested_cabinet_path"]),
        reasons=[str(reason) for reason in plan["reasons"]],
    )


@router.post(
    "/{document_id}/cabinet-confirm",
    response_model=DocumentCabinetConfirmResponse,
)
def confirm_document_cabinet(
    document_id: str,
    request: DocumentCabinetConfirmRequest,
) -> DocumentCabinetConfirmResponse:
    try:
        confirm_document_cabinet_path(document_id, request.cabinet_path)
    except ValueError as error:
        status_code = (
            status.HTTP_404_NOT_FOUND
            if str(error) == "Document not found."
            else status.HTTP_400_BAD_REQUEST
        )
        raise HTTPException(
            status_code=status_code,
            detail=str(error),
        ) from error

    return DocumentCabinetConfirmResponse(document=_get_document_or_404(document_id))
