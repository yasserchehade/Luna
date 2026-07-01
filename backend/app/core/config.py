from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    app_name: str = "Luna API"
    environment: str = "local"
    database_url: str = "postgresql+psycopg://luna:luna@localhost:5432/luna"
    redis_url: str = "redis://localhost:6379/0"
    file_storage_path: str = "./storage/documents"
    ai_provider: str = "stub"
    cors_origins: str = "http://localhost:3000"

    model_config = SettingsConfigDict(env_file=".env", extra="ignore")


settings = Settings()
