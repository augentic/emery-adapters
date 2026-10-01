from django.conf import settings
from rest_framework import serializers

from orders.models import Line, Order


class LineSerializer(serializers.ModelSerializer):
    class Meta:
        model = Line
        fields = ["sku", "quantity", "unit_cents"]


class OrderSerializer(serializers.ModelSerializer):
    lines = LineSerializer(many=True)
    total_cents = serializers.IntegerField(read_only=True)

    class Meta:
        model = Order
        fields = ["id", "status", "subtotal_cents", "shipping_cents", "total_cents", "note", "lines", "placed_at"]
        read_only_fields = ["status", "subtotal_cents", "shipping_cents", "placed_at"]

    def validate_lines(self, lines: list[dict]) -> list[dict]:
        if not lines:
            raise serializers.ValidationError("an order needs at least one line")
        if len(lines) > settings.ORDER_MAX_LINES:
            raise serializers.ValidationError(f"an order has at most {settings.ORDER_MAX_LINES} lines")
        return lines
