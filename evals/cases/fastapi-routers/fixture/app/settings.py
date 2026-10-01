from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="SHOPAPI_", env_file=".env")

    port: int = 8000
    database_url: str = "postgresql+psycopg://shop:shop@localhost:5432/shop"
    redis_url: str = "redis://localhost:6379/0"
    shipping_api_url: str = "https://rates.courier.example/v2"
    shipping_api_key: str = ""
    request_timeout_s: float = 5.0
    cache_ttl_s: int = 300
    order_cache_ttl_s: int = 60
    default_page_size: int = 20
    max_page_size: int = 100
    api_tokens: list[str] = Field(default_factory=list)
    cors_origins: list[str] = Field(default_factory=lambda: ["http://localhost:3000"])
    log_level: str = "info"


settings = Settings()
