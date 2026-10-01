from fastapi import APIRouter, Response
from sqlalchemy import text

from ..cache import cache
from ..db import engine

router = APIRouter(prefix="/health", tags=["health"])


@router.get("")
async def live(response: Response) -> dict:
    checks = {"database": _database(), "cache": await _cache()}
    if not all(checks.values()):
        response.status_code = 503
    return {"status": "ok" if all(checks.values()) else "degraded", "checks": checks}


@router.get("/ready")
async def ready() -> dict:
    return {"ready": True}


def _database() -> bool:
    try:
        with engine.connect() as connection:
            connection.execute(text("select 1"))
    except Exception:
        return False
    return True


async def _cache() -> bool:
    if cache.client is None:
        return False
    try:
        return bool(await cache.client.ping())
    except Exception:
        return False
