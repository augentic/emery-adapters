import pytest

from app.lib.errors import ConflictError, ValidationError
from app.models import Order
from app.services.orders import OrderService


class StubRepository:
    def __init__(self, order: Order) -> None:
        self.order = order
        self.updated = []

    def find(self, order_id):
        return self.order if order_id == self.order.id else None

    def update(self, order):
        self.updated.append(order)
        return order


def pending() -> Order:
    return Order(id="o-1", customer_id="c-1", state="pending", ship_to={"postcode": "1010"}, transitions=[], subtotal_cents=0, shipping_cents=0, total_cents=0)


@pytest.fixture(autouse=True)
def no_cache(monkeypatch):
    async def none(*args, **kwargs):
        return None

    monkeypatch.setattr("app.services.orders.cache.get", none)
    monkeypatch.setattr("app.services.orders.cache.set", none)
    monkeypatch.setattr("app.services.orders.cache.invalidate", none)


async def test_a_pending_order_can_be_paid():
    service = OrderService(StubRepository(pending()), None, None, None)
    order = await service.transition("o-1", "paid", None)
    assert order.state == "paid"
    assert order.transitions[-1]["state"] == "paid"


async def test_a_pending_order_cannot_be_shipped():
    service = OrderService(StubRepository(pending()), None, None, None)
    with pytest.raises(ConflictError):
        await service.transition("o-1", "shipped", None)


async def test_a_cancellation_needs_a_reason():
    service = OrderService(StubRepository(pending()), None, None, None)
    with pytest.raises(ValidationError):
        await service.transition("o-1", "cancelled", None)


async def test_a_delivered_order_is_final():
    order = pending()
    order.state = "delivered"
    service = OrderService(StubRepository(order), None, None, None)
    with pytest.raises(ConflictError):
        await service.transition("o-1", "cancelled", "changed my mind")


async def test_amending_a_paid_order_is_refused():
    order = pending()
    order.state = "paid"
    service = OrderService(StubRepository(order), None, None, None)
    with pytest.raises(ConflictError):
        await service.amend("o-1", None)
