from app.main import app


def test_fastapi_app_imports() -> None:
    assert app.title == "Luna API"
