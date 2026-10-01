from fastapi import APIRouter, Depends, Query, Response, status
from sqlalchemy.orm import Session

from ..deps import Authed, Db
from ..repositories.catalogue import Catalogue
from ..repositories.customers import CustomerRepository
from ..repositories.orders import OrderRepository
from ..schemas.orders import AmendLines, OrderOut, Page, PlaceOrder, Transition
from ..services.customers import CustomerService
from ..services.orders import OrderService
from ..services.shipping import ShippingClient
from ..settings import settings

router = APIRouter(prefix="/orders", tags=["orders"], dependencies=[Authed])

shipping = ShippingClient()


def service(db: Session = Db) -> OrderService:
    return OrderService(OrderRepository(db), Catalogue(db), CustomerService(CustomerRepository(db)), shipping)


def out(order) -> OrderOut:
    return OrderOut(
        id=order.id,
        customer_id=order.customer_id,
        state=order.state,
        lines=[{"sku": l.sku, "quantity": l.quantity, "unit_cents": l.unit_cents, "weight_grams": l.weight_grams} for l in order.lines],
        ship_to=order.ship_to,
        courier_note=order.courier_note,
        subtotal_cents=order.subtotal_cents,
        discount_cents=order.discount_cents,
        shipping_cents=order.shipping_cents,
        total_cents=order.total_cents,
        created_at=order.created_at.isoformat(),
        updated_at=order.updated_at.isoformat(),
    )


@router.get("", response_model=Page)
def list_orders(
    customer_id: str | None = None,
    state: str | None = None,
    page: int = Query(default=1, ge=1),
    size: int = Query(default=settings.default_page_size, ge=1, le=settings.max_page_size),
    orders: OrderService = Depends(service),
) -> Page:
    items, total = orders.list(customer_id=customer_id, state=state, page=page, size=size)
    return Page(items=[out(order) for order in items], total=total, page=page, size=size)


@router.post("", response_model=OrderOut, status_code=status.HTTP_201_CREATED)
async def place_order(request: PlaceOrder, response: Response, orders: OrderService = Depends(service)) -> OrderOut:
    order = await orders.place(request)
    response.headers["location"] = f"/api/v1/orders/{order.id}"
    return out(order)


@router.get("/{order_id}", response_model=OrderOut)
async def get_order(order_id: str, orders: OrderService = Depends(service)) -> OrderOut:
    return out(await orders.get(order_id))


@router.put("/{order_id}/lines", response_model=OrderOut)
async def amend_lines(order_id: str, request: AmendLines, orders: OrderService = Depends(service)) -> OrderOut:
    return out(await orders.amend(order_id, request))


@router.post("/{order_id}/transitions", response_model=OrderOut)
async def transition(order_id: str, request: Transition, orders: OrderService = Depends(service)) -> OrderOut:
    return out(await orders.transition(order_id, request.state, request.reason))
