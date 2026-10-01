import logging

import requests
from flask import current_app

from ..cache import cache
from .repository import Order, repository

log = logging.getLogger("shop.orders")

CANCELLABLE = ("pending", "paid")
PAGE_LIMIT = 50


class OrderError(Exception):
    status = 400
    code = "invalid"

    def __init__(self, message: str, status: int | None = None, code: str | None = None) -> None:
        super().__init__(message)
        if status is not None:
            self.status = status
        if code is not None:
            self.code = code


def validate(body: dict) -> tuple[str, list[dict]]:
    customer = str(body.get("customer") or "").strip()
    lines = body.get("lines")
    if not customer:
        raise OrderError("customer is required", code="customer_required")
    if not isinstance(lines, list) or not lines:
        raise OrderError("an order needs at least one line", code="lines_required")
    if len(lines) > current_app.config["MAX_LINES"]:
        raise OrderError(f"an order has at most {current_app.config['MAX_LINES']} lines", code="too_many_lines")
    for line in lines:
        if int(line.get("quantity", 0)) < 1 or int(line.get("unit_cents", 0)) < 0:
            raise OrderError("each line needs a positive quantity and a non-negative price", code="bad_line")
    return customer, lines


def price(lines: list[dict]) -> tuple[int, int]:
    subtotal = sum(int(line["quantity"]) * int(line["unit_cents"]) for line in lines)
    threshold = current_app.config["FREE_SHIPPING_THRESHOLD_CENTS"]
    shipping = 0 if subtotal >= threshold else current_app.config["SHIPPING_CENTS"]
    return subtotal + shipping, shipping


def notify(event: str, order: Order) -> None:
    url = current_app.config["WEBHOOK_URL"]
    if not url:
        return
    try:
        requests.post(
            url,
            json={"event": event, "order": order.as_dict()},
            timeout=current_app.config["WEBHOOK_TIMEOUT_S"],
        )
    except requests.RequestException as error:
        log.warning("webhook %s for order %s failed: %s", event, order.id, error)


def place(body: dict) -> Order:
    customer, lines = validate(body)
    total, shipping = price(lines)
    order = repository.insert(Order(0, customer, lines, "pending", total, shipping))
    cache.drop(f"orders:{customer}")
    notify("order.placed", order)
    return order


def read(order_id: int) -> Order:
    cached = cache.get(f"order:{order_id}")
    if cached is not None:
        return Order(**cached)
    order = repository.find(order_id)
    if order is None:
        raise OrderError(f"no order {order_id}", status=404, code="not_found")
    cache.put(f"order:{order_id}", order.as_dict())
    return order


def recent(customer: str, limit: int) -> list[Order]:
    limit = max(1, min(limit, PAGE_LIMIT))
    key = f"orders:{customer}"
    cached = cache.get(key)
    if cached is not None and len(cached) >= limit:
        return [Order(**item) for item in cached[:limit]]
    orders = repository.for_customer(customer, limit)
    cache.put(key, [order.as_dict() for order in orders])
    return orders


def cancel(order_id: int) -> Order:
    order = read(order_id)
    if order.status not in CANCELLABLE:
        raise OrderError(f"order is {order.status}", status=409, code="not_cancellable")
    repository.set_status(order_id, "cancelled")
    order.status = "cancelled"
    cache.drop(f"order:{order_id}")
    cache.drop(f"orders:{order.customer}")
    notify("order.cancelled", order)
    return order
