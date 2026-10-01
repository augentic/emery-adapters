from datetime import timedelta

from django.conf import settings
from django.core.mail import send_mail
from django.db import transaction
from django.utils import timezone

from .models import Line, Order


class OrderStateError(Exception):
    """A transition the order's state does not allow."""


def shipping_for(subtotal_cents: int) -> int:
    if subtotal_cents >= settings.FREE_SHIPPING_THRESHOLD_CENTS:
        return 0
    return settings.SHIPPING_CENTS


@transaction.atomic
def place(customer, lines: list[dict], note: str = "") -> Order:
    subtotal = sum(line["quantity"] * line["unit_cents"] for line in lines)
    order = Order.objects.create(
        customer=customer,
        subtotal_cents=subtotal,
        shipping_cents=shipping_for(subtotal),
        note=note,
    )
    Line.objects.bulk_create(Line(order=order, **line) for line in lines)
    send_mail(
        subject=f"Order {order.pk} received",
        message=f"Thanks — your order of {order.total_cents / 100:.2f} is pending.",
        from_email=settings.DEFAULT_FROM_EMAIL,
        recipient_list=[customer.email],
        fail_silently=True,
    )
    return order


def pay(order: Order) -> Order:
    if order.status != Order.Status.PENDING:
        raise OrderStateError(f"order is {order.status}")
    order.status = Order.Status.PAID
    order.save(update_fields=["status", "updated_at"])
    return order


def cancel(order: Order) -> Order:
    if order.status == Order.Status.SHIPPED:
        raise OrderStateError("a shipped order cannot be cancelled")
    window = timedelta(hours=settings.ORDER_CANCEL_WINDOW_HOURS)
    if order.status == Order.Status.PAID and timezone.now() - order.placed_at > window:
        raise OrderStateError("the cancellation window has closed")
    order.status = Order.Status.CANCELLED
    order.save(update_fields=["status", "updated_at"])
    send_mail(
        subject=f"Order {order.pk} cancelled",
        message="Your order was cancelled; any payment will be refunded.",
        from_email=settings.DEFAULT_FROM_EMAIL,
        recipient_list=[order.customer.email],
        fail_silently=True,
    )
    return order
