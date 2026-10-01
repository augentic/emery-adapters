from django.contrib.auth import get_user_model
from django.test import TestCase, override_settings
from django.utils import timezone

from . import services
from .models import Order


class OrderServiceTests(TestCase):
    def setUp(self):
        self.customer = get_user_model().objects.create_user("ada", "ada@example.com", "pw")

    def test_an_order_under_the_threshold_pays_shipping(self):
        order = services.place(self.customer, [{"sku": "A", "quantity": 1, "unit_cents": 1200}])
        self.assertEqual(order.shipping_cents, 850)
        self.assertEqual(order.total_cents, 2050)

    def test_an_order_at_the_threshold_ships_free(self):
        order = services.place(self.customer, [{"sku": "A", "quantity": 1, "unit_cents": 10_000}])
        self.assertEqual(order.shipping_cents, 0)

    def test_paying_a_paid_order_is_refused(self):
        order = services.place(self.customer, [{"sku": "A", "quantity": 1, "unit_cents": 100}])
        services.pay(order)
        with self.assertRaises(services.OrderStateError):
            services.pay(order)

    @override_settings(ORDER_CANCEL_WINDOW_HOURS=0)
    def test_a_paid_order_past_the_window_cannot_be_cancelled(self):
        """Once the cancellation window has closed, a paid order stays paid."""
        order = services.place(self.customer, [{"sku": "A", "quantity": 1, "unit_cents": 100}])
        services.pay(order)
        Order.objects.filter(pk=order.pk).update(placed_at=timezone.now() - timezone.timedelta(hours=1))
        order.refresh_from_db()
        with self.assertRaises(services.OrderStateError):
            services.cancel(order)

    def test_cancelling_requires_confirmation(self):
        self.client.force_login(self.customer)
        order = services.place(self.customer, [{"sku": "A", "quantity": 1, "unit_cents": 100}])
        response = self.client.post(f"/orders/{order.pk}/cancel/", {})
        self.assertEqual(response.status_code, 400)
