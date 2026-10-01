from collections.abc import Iterator

from fastapi import Depends, Header, HTTPException
from sqlalchemy.orm import Session

from .db import session
from .settings import settings


def bearer(authorization: str = Header(default="")) -> str:
    token = authorization.removeprefix("Bearer ").strip()
    if not token or token not in settings.api_tokens:
        raise HTTPException(status_code=401, detail="missing or unknown bearer token")
    return token


def db() -> Iterator[Session]:
    yield from session()


Authed = Depends(bearer)
Db = Depends(db)
