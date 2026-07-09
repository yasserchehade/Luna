from typing import Annotated

from fastapi import APIRouter, File, HTTPException, UploadFile, status
from psycopg.types.json import Jsonb

from app.db import get_connection, get_default_workspace_id
from app.models.document import (
    Document,
    DocumentCabinetPlan,
    DocumentCabinetConfirmRequest,
    DocumentCabinetConfirmResponse,
    DocumentCabinetFileRequest,
    DocumentCabinetFileResponse,
    DocumentSearchResult,
    DocumentText,
)
from app.services.cabinet import (
    confirm_document_cabinet_path,
    file_document_in_cabinet,
    save_document_cabinet_plan,
)
from app.services.document_text import extract_pdf_text
from app.services.audit import record_audit_event
from app.models.work import Capability, WorkOrderCreate
from app.services.work import mark_work_executed, prepare_user_approved_work
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
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="document.uploaded",
                entity_type="document",
                entity_id=stored.document_id,
                metadata={
                    "original_filename": stored.original_filename,
                    "content_type": stored.content_type,
                    "storage_provider": "local_folder",
                    "size_bytes": stored.size_bytes,
                    "text_characters": extracted_text.character_count,
                },
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


@router.get("/search", response_model=list[DocumentSearchResult])
def search_documents(query: str) -> list[DocumentSearchResult]:
    normalized_query = query.strip()
    if len(normalized_query) < 2:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Search query must be at least 2 characters.",
        )

    like_query = f"%{normalized_query}%"

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
                    t.character_count,
                    ts_headline(
                        'english',
                        COALESCE(t.text_content, ''),
                        plainto_tsquery('english', %s),
                        'MaxWords=18, MinWords=6, ShortWord=2'
                    ) AS snippet,
                    b.supplier,
                    b.invoice_number,
                    b.category
                FROM documents d
                LEFT JOIN document_texts t ON t.document_id = d.id
                LEFT JOIN bills b ON b.document_id = d.id
                WHERE
                    to_tsvector(
                        'english',
                        concat_ws(
                            ' ',
                            d.original_filename,
                            d.suggested_cabinet_path,
                            d.confirmed_cabinet_path,
                            COALESCE(t.text_content, ''),
                            b.supplier,
                            b.invoice_number,
                            b.category
                        )
                    ) @@ plainto_tsquery('english', %s)
                    OR d.original_filename ILIKE %s
                    OR d.suggested_cabinet_path ILIKE %s
                    OR d.confirmed_cabinet_path ILIKE %s
                    OR b.supplier ILIKE %s
                    OR b.invoice_number ILIKE %s
                    OR b.category ILIKE %s
                ORDER BY d.received_at DESC
                LIMIT 20
                """,
                (
                    normalized_query,
                    normalized_query,
                    like_query,
                    like_query,
                    like_query,
                    like_query,
                    like_query,
                    like_query,
                ),
            )
            rows = cursor.fetchall()

    return [
        DocumentSearchResult(
            document=_document_from_row(row),
            supplier=row["supplier"],
            invoice_number=row["invoice_number"],
            category=row["category"],
            snippet=row["snippet"],
        )
        for row in rows
    ]


@router.get("/{document_id}", response_model=Document)
def get_document(document_id: str) -> Document:
    document = _get_document_or_404(document_id)
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="document.viewed",
                entity_type="document",
                entity_id=document_id,
                metadata={"original_filename": document.original_filename},
            )
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
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="document.cabinet_plan_suggested",
                entity_type="document",
                entity_id=document_id,
                metadata={
                    "storage_provider": str(plan["storage_provider"]),
                    "suggested_cabinet_path": str(plan["suggested_cabinet_path"]),
                    "reasons": [str(reason) for reason in plan["reasons"]],
                },
            )
            work_order = prepare_user_approved_work(
                cursor,
                workspace_id=workspace_id,
                request=WorkOrderCreate(
                    work_type="document.cabinet_plan",
                    title="Prepare cabinet filing plan",
                    capability_required=Capability.write,
                    subject_entity_type="document",
                    subject_entity_id=document_id,
                    source_document_id=document_id,
                    evidence={
                        "suggested_cabinet_path": str(plan["suggested_cabinet_path"]),
                        "reasons": [str(reason) for reason in plan["reasons"]],
                    },
                ),
                approval_reason="Luna prepared a cabinet path suggestion for review.",
            )
            mark_work_executed(
                cursor,
                workspace_id=workspace_id,
                work_order_id=work_order.id,
                result={"cabinet_status": str(plan["cabinet_status"])},
            )

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

    document = _get_document_or_404(document_id)
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="document.cabinet_path_confirmed",
                entity_type="document",
                entity_id=document_id,
                metadata={
                    "confirmed_cabinet_path": document.confirmed_cabinet_path,
                    "user_supplied_path": request.cabinet_path is not None,
                },
            )
            work_order = prepare_user_approved_work(
                cursor,
                workspace_id=workspace_id,
                request=WorkOrderCreate(
                    work_type="document.cabinet_confirm",
                    title="Confirm cabinet filing path",
                    capability_required=Capability.write,
                    subject_entity_type="document",
                    subject_entity_id=document_id,
                    source_document_id=document_id,
                    evidence={
                        "confirmed_cabinet_path": document.confirmed_cabinet_path,
                        "user_supplied_path": request.cabinet_path is not None,
                    },
                ),
                approval_reason="Confirming a cabinet path approves Luna's filing plan.",
            )
            mark_work_executed(
                cursor,
                workspace_id=workspace_id,
                work_order_id=work_order.id,
                result={"cabinet_status": document.cabinet_status},
            )

    return DocumentCabinetConfirmResponse(document=document)


@router.post(
    "/{document_id}/cabinet-file",
    response_model=DocumentCabinetFileResponse,
)
def file_document_cabinet(
    document_id: str,
    request: DocumentCabinetFileRequest,
) -> DocumentCabinetFileResponse:
    try:
        result = file_document_in_cabinet(document_id, request.mode)
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

    document = _get_document_or_404(document_id)
    with get_connection() as connection:
        workspace_id = get_default_workspace_id(connection)
        with connection.cursor() as cursor:
            record_audit_event(
                cursor,
                workspace_id=workspace_id,
                event_type="document.cabinet_filed",
                entity_type="document",
                entity_id=document_id,
                metadata={
                    "mode": result["mode"],
                    "source_path": result["source_path"],
                    "filed_path": result["filed_path"],
                },
            )
            work_order = prepare_user_approved_work(
                cursor,
                workspace_id=workspace_id,
                request=WorkOrderCreate(
                    work_type="document.cabinet_file",
                    title="File document in household cabinet",
                    capability_required=Capability.write,
                    subject_entity_type="document",
                    subject_entity_id=document_id,
                    source_document_id=document_id,
                    evidence={
                        "mode": result["mode"],
                        "source_path": result["source_path"],
                        "filed_path": result["filed_path"],
                    },
                ),
                approval_reason="Filing a document changes the household cabinet.",
            )
            mark_work_executed(
                cursor,
                workspace_id=workspace_id,
                work_order_id=work_order.id,
                result={
                    "mode": result["mode"],
                    "filed_path": result["filed_path"],
                },
            )

    return DocumentCabinetFileResponse(
        document=document,
        source_path=str(result["source_path"]),
        filed_path=str(result["filed_path"]),
        mode=str(result["mode"]),
    )
