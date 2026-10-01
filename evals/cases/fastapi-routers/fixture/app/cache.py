import json
from typing import Any

from redis.asyncio import Redis

from .settings import settings


class Cache:
    """JSON values in Redis under a TTL; absent until `connect`, silent after `close`."""

    def __init__(self) -> None:
        self.client: Redis | None = None

    async def connect(self) -> None:
        self.client = Redis.from_url(settings.redis_url, decode_responses=True)
        await self.client.ping()

    async def close(self) -> None:
        if self.client is not None:
            await self.client.aclose()
            self.client = None

    async def get(self, key: str) -> Any | None:
        if self.client is None:
            return None
        raw = await self.client.get(key)
        return None if raw is None else json.loads(raw)

    async def set(self, key: str, value: Any, ttl_s: int | None = None) -> None:
        if self.client is None:
            return
        await self.client.set(key, json.dumps(value, default=str), ex=ttl_s or settings.cache_ttl_s)

    async def invalidate(self, *keys: str) -> None:
        if self.client is not None and keys:
            await self.client.delete(*keys)


cache = Cache()
