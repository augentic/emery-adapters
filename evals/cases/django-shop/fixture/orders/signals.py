import logging

from django.db.models.signals import post_save
from django.dispatch import receiver

from .models import Order

log = logging.getLogger("orders")


@receiver(post_save, sender=Order)
def order_saved(sender, instance: Order, created: bool, **kwargs) -> None:
    log.info("order %s %s (%s)", instance.pk, "placed" if created else "updated", instance.status)
