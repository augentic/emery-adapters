from django.conf import settings
from django.db import models


class Order(models.Model):
    class Status(models.TextChoices):
        PENDING = "pending", "Pending"
        PAID = "paid", "Paid"
        SHIPPED = "shipped", "Shipped"
        CANCELLED = "cancelled", "Cancelled"

    customer = models.ForeignKey(settings.AUTH_USER_MODEL, on_delete=models.PROTECT, related_name="orders")
    status = models.CharField(max_length=16, choices=Status.choices, default=Status.PENDING)
    subtotal_cents = models.PositiveIntegerField(default=0)
    shipping_cents = models.PositiveIntegerField(default=0)
    note = models.CharField(max_length=280, blank=True)
    placed_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        ordering = ["-placed_at"]

    @property
    def total_cents(self) -> int:
        return self.subtotal_cents + self.shipping_cents

    def __str__(self) -> str:
        return f"Order {self.pk} ({self.status})"


class Line(models.Model):
    order = models.ForeignKey(Order, on_delete=models.CASCADE, related_name="lines")
    sku = models.CharField(max_length=32)
    quantity = models.PositiveIntegerField()
    unit_cents = models.PositiveIntegerField()

    @property
    def amount_cents(self) -> int:
        return self.quantity * self.unit_cents
