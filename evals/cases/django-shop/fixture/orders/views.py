from django.contrib.auth.decorators import login_required
from django.contrib.auth.mixins import LoginRequiredMixin
from django.http import HttpResponseBadRequest, JsonResponse
from django.shortcuts import get_object_or_404
from django.views import View
from django.views.decorators.http import require_POST

from . import services
from .forms import OrderForm
from .models import Order

PAGE_SIZE = 20


def serialize(order: Order) -> dict:
    return {
        "id": order.pk,
        "status": order.status,
        "subtotal_cents": order.subtotal_cents,
        "shipping_cents": order.shipping_cents,
        "total_cents": order.total_cents,
        "note": order.note,
        "lines": [{"sku": line.sku, "quantity": line.quantity, "unit_cents": line.unit_cents} for line in order.lines.all()],
    }


@login_required
def list_orders(request):
    orders = Order.objects.filter(customer=request.user).prefetch_related("lines")[:PAGE_SIZE]
    return JsonResponse({"orders": [serialize(order) for order in orders]})


@login_required
@require_POST
def place_order(request):
    form = OrderForm(request.POST)
    if not form.is_valid():
        return JsonResponse({"errors": form.errors}, status=400)
    order = services.place(request.user, form.cleaned_data["lines"], form.cleaned_data["note"])
    return JsonResponse(serialize(order), status=201)


@login_required
def order_detail(request, pk: int):
    order = get_object_or_404(Order, pk=pk, customer=request.user)
    return JsonResponse(serialize(order))


class PayView(LoginRequiredMixin, View):
    def post(self, request, pk: int):
        order = get_object_or_404(Order, pk=pk, customer=request.user)
        try:
            services.pay(order)
        except services.OrderStateError as error:
            return JsonResponse({"error": str(error)}, status=409)
        return JsonResponse(serialize(order))


class CancelView(LoginRequiredMixin, View):
    def post(self, request, pk: int):
        order = get_object_or_404(Order, pk=pk, customer=request.user)
        if request.POST.get("confirm") != "yes":
            return HttpResponseBadRequest("confirm=yes is required")
        try:
            services.cancel(order)
        except services.OrderStateError as error:
            return JsonResponse({"error": str(error)}, status=409)
        return JsonResponse(serialize(order))
