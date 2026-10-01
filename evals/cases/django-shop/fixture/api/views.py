from rest_framework import status, viewsets
from rest_framework.decorators import action
from rest_framework.response import Response

from orders import services
from orders.models import Order

from .serializers import OrderSerializer


class OrderViewSet(viewsets.ModelViewSet):
    serializer_class = OrderSerializer
    http_method_names = ["get", "post", "head", "options"]

    def get_queryset(self):
        return Order.objects.filter(customer=self.request.user).prefetch_related("lines")

    def perform_create(self, serializer: OrderSerializer) -> None:
        data = serializer.validated_data
        serializer.instance = services.place(self.request.user, data["lines"], data.get("note", ""))

    @action(detail=True, methods=["post"])
    def pay(self, request, pk=None):
        order = self.get_object()
        try:
            services.pay(order)
        except services.OrderStateError as error:
            return Response({"detail": str(error)}, status=status.HTTP_409_CONFLICT)
        return Response(self.get_serializer(order).data)

    @action(detail=True, methods=["post"])
    def cancel(self, request, pk=None):
        order = self.get_object()
        try:
            services.cancel(order)
        except services.OrderStateError as error:
            return Response({"detail": str(error)}, status=status.HTTP_409_CONFLICT)
        return Response(self.get_serializer(order).data)
