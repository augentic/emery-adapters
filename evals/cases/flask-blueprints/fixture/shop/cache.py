import json
from typing import Any

import redis
from flask import Flask


class Cache:
    """A JSON cache over Redis with one TTL for every key; no client until `init_app`."""

    def __init__(self) -> None:
        self.client: redis.Redis | None = None
        self.ttl_s = 0

    def init_app(self, app: Flask) -> None:
        self.client = redis.Redis.from_url(app.config["REDIS_URL"], decode_responses=True)
        self.ttl_s = app.config["CACHE_TTL_S"]

    def get(self, key: str) -> Any | None:
        if self.client is None:
            return None
        raw = self.client.get(key)
        return None if raw is None else json.loads(raw)

    def put(self, key: str, value: Any) -> None:
        if self.client is None:
            return
        self.client.set(key, json.dumps(value), ex=self.ttl_s)

    def drop(self, key: str) -> None:
        if self.client is not None:
            self.client.delete(key)


cache = Cache()
