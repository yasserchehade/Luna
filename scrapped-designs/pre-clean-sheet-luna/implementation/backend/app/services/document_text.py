from dataclasses import dataclass, field
from pathlib import Path

from pypdf import PdfReader


@dataclass(frozen=True)
class ExtractedDocumentText:
    text_content: str
    extraction_method: str
    page_count: int
    character_count: int
    metadata: dict[str, object] = field(default_factory=dict)


def extract_pdf_text(storage_path: str) -> ExtractedDocumentText:
    path = Path(storage_path)
    try:
        reader = PdfReader(path)
    except Exception as error:
        return ExtractedDocumentText(
            text_content="",
            extraction_method="pypdf",
            page_count=0,
            character_count=0,
            metadata={"filename": path.name, "error": str(error)},
        )

    pages: list[str] = []
    failed_pages: list[dict[str, object]] = []

    for page_number, page in enumerate(reader.pages, start=1):
        try:
            page_text = page.extract_text() or ""
        except Exception as error:  # pypdf can fail on malformed individual pages.
            page_text = ""
            failed_pages.append({"page": page_number, "error": str(error)})
        if page_text:
            pages.append(page_text.strip())

    text_content = "\n\n".join(part for part in pages if part).strip()
    return ExtractedDocumentText(
        text_content=text_content,
        extraction_method="pypdf",
        page_count=len(reader.pages),
        character_count=len(text_content),
        metadata={"filename": path.name, "failed_pages": failed_pages},
    )
