from typing import Literal

from pydantic import BaseModel, Field, field_validator

MAX_LINES = 50
MAX_QUANTITY = 999

OrderState = Literal["pending", "paid", "packed", "shipped", "delivered", "cancelled"]


class LineIn(BaseModel):
    sku: str = Field(min_length=1, max_length=32, pattern=r"^[A-Z0-9-]+$")
    quantity: int = Field(ge=1, le=MAX_QUANTITY)


class Address(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    line1: str = Field(min_length=1, max_length=120)
    line2: str | None = Field(default=None, max_length=120)
    city: str = Field(min_length=1, max_length=80)
    postcode: str = Field(pattern=r"^\d{4}$")


class PlaceOrder(BaseModel):
    customer_id: str
    lines: list[LineIn] = Field(min_length=1, max_length=MAX_LINES)
    ship_to: Address
    courier_note: str | None = Field(default=None, max_length=280)

    @field_validator("lines")
    @classmethod
    def skus_unique(cls, lines: list[LineIn]) -> list[LineIn]:
        skus = [line.sku for line in lines]
        if len(set(skus)) != len(skus):
            raise ValueError("each sku appears once")
        return lines


class AmendLines(BaseModel):
    lines: list[LineIn] = Field(min_length=1, max_length=MAX_LINES)


class Transition(BaseModel):
    state: OrderState
    reason: str | None = Field(default=None, max_length=280)


class LineOut(BaseModel):
    sku: str
    quantity: int
    unit_cents: int
    weight_grams: int


class OrderOut(BaseModel):
    id: str
    customer_id: str
    state: OrderState
    lines: list[LineOut]
    ship_to: Address
    courier_note: str | None
    subtotal_cents: int
    discount_cents: int
    shipping_cents: int
    total_cents: int
    created_at: str
    updated_at: str


class Page(BaseModel):
    items: list[OrderOut]
    total: int
    page: int
    size: int
