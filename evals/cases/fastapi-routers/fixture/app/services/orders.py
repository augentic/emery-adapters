import logging
from datetime import UTC, datetime
from uuid import uuid4

from ..cache import cache
from ..lib.errors import ConflictError, NotFoundError, ValidationError
from ..models import TERMINAL_STATES, TRANSITIONS, Line, Order
from ..repositories.catalogue import Catalogue, Priced
from ..repositories.orders import OrderRepository
from ..schemas.orders import AmendLines, LineIn, PlaceOrder
from ..settings import settings
from .customers import CustomerService
from .pricing import totals, weight
from .shipping import ShippingClient

log = logging.getLogger("shopapi.orders")


class OrderService:
    def __init__(self, repository: OrderRepository, catalogue: Catalogue, customers: CustomerService, shipping: ShippingClient) -> None:
        self.repository = repository
        self.catalogue = catalogue
        self.customers = customers
        self.shipping = shipping

    async def get(self, order_id: str) -> Order:
        cached = await cache.get(f"order:{order_id}")
        if cached is not None:
            return _load(cached)
        order = self.repository.find(order_id)
        if order is None:
            raise NotFoundError("order", order_id)
        await cache.set(f"order:{order_id}", _dump(order), settings.order_cache_ttl_s)
        return order

    def list(self, *, customer_id: str | None, state: str | None, page: int, size: int) -> tuple[list[Order], int]:
        return self.repository.list(customer_id=customer_id, state=state, page=page, size=size)

    async def place(self, request: PlaceOrder) -> Order:
        customer = await self.customers.get(request.customer_id)
        priced = self.price(request.lines)
        quote = await self.shipping.quote(request.ship_to.postcode, weight(priced))
        now = datetime.now(UTC)
        order = Order(
            id=str(uuid4()),
            customer_id=customer.id,
            state="pending",
            ship_to=request.ship_to.model_dump(),
            courier_note=request.courier_note,
            transitions=[{"state": "pending", "at": now.isoformat()}],
            created_at=now,
            updated_at=now,
            **totals(priced, customer.tier, quote["cents"], request.ship_to.postcode),
        )
        order.lines = [Line(sku=p.sku, quantity=p.quantity, unit_cents=p.unit_cents, weight_grams=p.weight_grams) for p in priced]
        self.repository.insert(order)
        log.info("order %s placed for %s at %s cents", order.id, customer.id, order.total_cents)
        return order

    async def amend(self, order_id: str, request: AmendLines) -> Order:
        order = await self.get(order_id)
        if order.state != "pending":
            raise ConflictError(order.state, f"order {order_id} is {order.state} and its lines are fixed")
        customer = await self.customers.get(order.customer_id)
        priced = self.price(request.lines)
        quote = await self.shipping.quote(order.ship_to["postcode"], weight(priced))
        order.lines = [Line(sku=p.sku, quantity=p.quantity, unit_cents=p.unit_cents, weight_grams=p.weight_grams) for p in priced]
        for key, value in totals(priced, customer.tier, quote["cents"], order.ship_to["postcode"]).items():
            setattr(order, key, value)
        order.updated_at = datetime.now(UTC)
        self.repository.update(order)
        await cache.invalidate(f"order:{order_id}")
        return order

    async def transition(self, order_id: str, state: str, reason: str | None) -> Order:
        order = await self.get(order_id)
        if order.state in TERMINAL_STATES:
            raise ConflictError(order.state, f"order {order_id} is {order.state} and final")
        if state not in TRANSITIONS[order.state]:
            raise ConflictError(order.state, f"order {order_id} cannot go from {order.state} to {state}")
        if state == "cancelled" and not reason:
            raise ValidationError("reason", "a cancellation states its reason")
        now = datetime.now(UTC)
        order.state = state
        order.transitions = [*order.transitions, {"state": state, "at": now.isoformat(), "reason": reason}]
        order.updated_at = now
        self.repository.update(order)
        await cache.invalidate(f"order:{order_id}")
        log.info("order %s is now %s", order.id, state)
        return order

    def price(self, lines: list[LineIn]) -> list[Priced]:
        products = self.catalogue.products([line.sku for line in lines])
        priced: list[Priced] = []
        for line in lines:
            product = products.get(line.sku)
            if product is None:
                raise ValidationError("lines", f"unknown sku {line.sku}")
            if product.stock < line.quantity:
                raise ConflictError("stock", f"only {product.stock} of {line.sku} in stock")
            priced.append(Priced(line.sku, line.quantity, product.unit_cents, product.weight_grams))
        return priced


def _dump(order: Order) -> dict:
    return {
        "id": order.id,
        "customer_id": order.customer_id,
        "state": order.state,
        "ship_to": order.ship_to,
        "courier_note": order.courier_note,
        "subtotal_cents": order.subtotal_cents,
        "discount_cents": order.discount_cents,
        "shipping_cents": order.shipping_cents,
        "total_cents": order.total_cents,
        "transitions": order.transitions,
        "created_at": order.created_at,
        "updated_at": order.updated_at,
        "lines": [
            {"sku": line.sku, "quantity": line.quantity, "unit_cents": line.unit_cents, "weight_grams": line.weight_grams}
            for line in order.lines
        ],
    }


def _load(data: dict) -> Order:
    lines = data.pop("lines")
    order = Order(**{key: value for key, value in data.items() if key not in ("created_at", "updated_at")})
    order.created_at = datetime.fromisoformat(data["created_at"])
    order.updated_at = datetime.fromisoformat(data["updated_at"])
    order.lines = [Line(**line) for line in lines]
    return order
