"""Orders API: one FastAPI module for placing, reading, paying, and cancelling orders."""

from __future__ import annotations

import os
from datetime import datetime, timezone
from enum import Enum

import httpx
from fastapi import Depends, FastAPI, Header, HTTPException
from pydantic import BaseModel, Field

MAX_LINES = 50
TAX_RATE = float(os.getenv("TAX_RATE", "0.15"))
PRICING_URL = os.getenv("PRICING_URL", "https://pricing.example/v1")
PRICING_TIMEOUT_S = float(os.getenv("PRICING_TIMEOUT_S", "2.5"))
API_TOKENS = {token for token in os.getenv("API_TOKENS", "dev-token").split(",") if token}
CANCEL_WINDOW_HOURS = 24

app = FastAPI(title="Orders API", version="1.4.0")


class Status(str, Enum):
    PENDING = "pending"
    PAID = "paid"
    SHIPPED = "shipped"
    CANCELLED = "cancelled"


class Line(BaseModel):
    sku: str = Field(min_length=1)
    quantity: int = Field(ge=1)


class OrderIn(BaseModel):
    customer_id: str
    lines: list[Line]
    note: str | None = None


class Order(BaseModel):
    id: str
    customer_id: str
    lines: list[Line]
    status: Status
    subtotal_cents: int
    tax_cents: int
    total_cents: int
    placed_at: datetime


ORDERS: dict[str, Order] = {}
_sequence = 0


def require_token(authorization: str = Header(default="")) -> str:
    token = authorization.removeprefix("Bearer ").strip()
    if token not in API_TOKENS:
        raise HTTPException(status_code=401, detail="missing or unknown bearer token")
    return token


async def price_lines(lines: list[Line]) -> int:
    payload = [{"sku": line.sku, "quantity": line.quantity} for line in lines]
    async with httpx.AsyncClient(timeout=PRICING_TIMEOUT_S) as client:
        response = await client.post(f"{PRICING_URL}/quote", json=payload)
    if response.status_code != 200:
        raise HTTPException(status_code=502, detail="pricing service unavailable")
    return int(response.json()["subtotal_cents"])


def next_id() -> str:
    global _sequence
    _sequence += 1
    return f"ord_{_sequence}"


def stored(order_id: str) -> Order:
    order = ORDERS.get(order_id)
    if order is None:
        raise HTTPException(status_code=404, detail=f"no order {order_id}")
    return order


@app.post("/orders", status_code=201)
async def place_order(body: OrderIn, _: str = Depends(require_token)) -> Order:
    if not body.lines:
        raise HTTPException(status_code=422, detail="an order needs at least one line")
    if len(body.lines) > MAX_LINES:
        raise HTTPException(status_code=422, detail=f"an order has at most {MAX_LINES} lines")
    subtotal = await price_lines(body.lines)
    tax = round(subtotal * TAX_RATE)
    order = Order(
        id=next_id(),
        customer_id=body.customer_id,
        lines=body.lines,
        status=Status.PENDING,
        subtotal_cents=subtotal,
        tax_cents=tax,
        total_cents=subtotal + tax,
        placed_at=datetime.now(timezone.utc),
    )
    ORDERS[order.id] = order
    return order


@app.get("/orders/{order_id}")
async def read_order(order_id: str, _: str = Depends(require_token)) -> Order:
    return stored(order_id)


@app.post("/orders/{order_id}/pay")
async def pay_order(order_id: str, _: str = Depends(require_token)) -> Order:
    order = stored(order_id)
    if order.status is not Status.PENDING:
        raise HTTPException(status_code=409, detail=f"order is {order.status.value}")
    paid = order.model_copy(update={"status": Status.PAID})
    ORDERS[order_id] = paid
    return paid


@app.post("/orders/{order_id}/cancel")
async def cancel_order(order_id: str, _: str = Depends(require_token)) -> Order:
    order = stored(order_id)
    if order.status is Status.SHIPPED:
        raise HTTPException(status_code=409, detail="a shipped order cannot be cancelled")
    age = datetime.now(timezone.utc) - order.placed_at
    if order.status is Status.PAID and age.total_seconds() > CANCEL_WINDOW_HOURS * 3600:
        raise HTTPException(status_code=409, detail="the cancellation window has closed")
    cancelled = order.model_copy(update={"status": Status.CANCELLED})
    ORDERS[order_id] = cancelled
    return cancelled


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=int(os.getenv("PORT", "8000")))
