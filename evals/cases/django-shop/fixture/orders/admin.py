from django.contrib import admin

from .models import Line, Order


class LineInline(admin.TabularInline):
    model = Line
    extra = 0


@admin.register(Order)
class OrderAdmin(admin.ModelAdmin):
    list_display = ("id", "customer", "status", "total_cents", "placed_at")
    list_filter = ("status",)
    inlines = [LineInline]
