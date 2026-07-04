from app.core.config import settings
from app.services.cabinet import (
    _cabinet_destination_path,
    _safe_cabinet_path,
    _unique_destination_path,
)


def test_safe_cabinet_path_sanitizes_segments() -> None:
    assert (
        _safe_cabinet_path('Trust/Property: One/Invoice * 123?.pdf')
        == "Trust/Property-One/Invoice-123.pdf"
    )


def test_cabinet_destination_stays_under_configured_root(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(settings, "cabinet_storage_path", str(tmp_path))

    destination = _cabinet_destination_path("../Trust/../Property/Bill.pdf")

    assert destination == tmp_path / "Trust" / "Property" / "Bill.pdf"


def test_unique_destination_path_does_not_overwrite_existing_file(tmp_path) -> None:
    destination = tmp_path / "Bill.pdf"
    destination.write_text("existing", encoding="utf-8")

    unique = _unique_destination_path(destination)

    assert unique == tmp_path / "Bill-2.pdf"
