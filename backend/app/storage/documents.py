from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from uuid import uuid4

from fastapi import UploadFile

from app.core.config import settings

MAX_UPLOAD_BYTES = 10 * 1024 * 1024


@dataclass(frozen=True)
class StoredDocument:
    document_id: str
    original_filename: str
    content_type: str
    storage_path: str
    sha256: str
    size_bytes: int


async def store_uploaded_document(file: UploadFile) -> StoredDocument:
    content = await file.read()
    if len(content) > MAX_UPLOAD_BYTES:
        raise ValueError("Uploaded document is larger than 10 MB.")

    digest = sha256(content).hexdigest()
    document_id = str(uuid4())
    original_filename = Path(file.filename or "document.pdf").name
    content_type = file.content_type or "application/pdf"

    storage_root = Path(settings.file_storage_path)
    storage_root.mkdir(parents=True, exist_ok=True)

    extension = Path(original_filename).suffix.lower() or ".pdf"
    storage_path = storage_root / f"{document_id}{extension}"
    storage_path.write_bytes(content)

    return StoredDocument(
        document_id=document_id,
        original_filename=original_filename,
        content_type=content_type,
        storage_path=str(storage_path),
        sha256=digest,
        size_bytes=len(content),
    )
