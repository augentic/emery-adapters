import os


class Config:
    SECRET_KEY = os.environ.get("SECRET_KEY", "dev-only-change-me")
    REDIS_URL = os.environ.get("REDIS_URL", "redis://localhost:6379/1")
    WEBHOOK_URL = os.environ.get("ORDER_WEBHOOK_URL", "https://hooks.example/orders")
    WEBHOOK_TIMEOUT_S = float(os.environ.get("ORDER_WEBHOOK_TIMEOUT_S", "5"))
    CACHE_TTL_S = int(os.environ.get("CACHE_TTL_S", "300"))
    TOKEN_TTL_S = int(os.environ.get("TOKEN_TTL_S", "3600"))
    MAX_LINES = 20
    FREE_SHIPPING_THRESHOLD_CENTS = 10_000
    SHIPPING_CENTS = 850
    JSON_SORT_KEYS = False
