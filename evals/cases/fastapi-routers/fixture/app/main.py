import logging
import time
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from .cache import cache
from .lib.errors import ConflictError, NotFoundError, UpstreamError, ValidationError
from .routers import customers, health, orders
from .settings import settings

log = logging.getLogger("shopapi")


@asynccontextmanager
async def lifespan(app: FastAPI):
    logging.basicConfig(level=settings.log_level.upper())
    await cache.connect()
    log.info("shopapi ready on port %s", settings.port)
    yield
    await orders.shipping.close()
    await cache.close()


app = FastAPI(title="shopapi", version="1.2.0", lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.cors_origins,
    allow_methods=["GET", "POST", "PUT"],
    allow_headers=["authorization", "content-type"],
)


@app.middleware("http")
async def timing(request: Request, call_next):
    started = time.perf_counter()
    response = await call_next(request)
    response.headers["server-timing"] = f"app;dur={(time.perf_counter() - started) * 1000:.1f}"
    return response


@app.exception_handler(NotFoundError)
async def not_found(request: Request, error: NotFoundError) -> JSONResponse:
    return JSONResponse(status_code=404, content={"error": "not_found", "detail": str(error)})


@app.exception_handler(ConflictError)
async def conflict(request: Request, error: ConflictError) -> JSONResponse:
    return JSONResponse(status_code=409, content={"error": "conflict", "state": error.state, "detail": str(error)})


@app.exception_handler(ValidationError)
async def invalid(request: Request, error: ValidationError) -> JSONResponse:
    return JSONResponse(status_code=422, content={"error": "validation", "field": error.field, "detail": str(error)})


@app.exception_handler(UpstreamError)
async def upstream(request: Request, error: UpstreamError) -> JSONResponse:
    log.error("upstream failure: %s", error)
    return JSONResponse(status_code=502, content={"error": "upstream", "system": error.system, "detail": str(error)})


app.include_router(health.router)
app.include_router(orders.router, prefix="/api/v1")
app.include_router(customers.router, prefix="/api/v1")


def serve() -> None:
    uvicorn.run("app.main:app", host="0.0.0.0", port=settings.port, log_level=settings.log_level)
