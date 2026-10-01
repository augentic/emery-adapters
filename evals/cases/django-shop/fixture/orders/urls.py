from django.urls import path

from . import views

app_name = "orders"

urlpatterns = [
    path("", views.list_orders, name="list"),
    path("new/", views.place_order, name="place"),
    path("<int:pk>/", views.order_detail, name="detail"),
    path("<int:pk>/pay/", views.PayView.as_view(), name="pay"),
    path("<int:pk>/cancel/", views.CancelView.as_view(), name="cancel"),
]
