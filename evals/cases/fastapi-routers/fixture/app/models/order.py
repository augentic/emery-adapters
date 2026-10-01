from datetime import datetime

from sqlalchemy import JSON, DateTime, ForeignKey, Integer, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from ..db import Base

STATES = ("pending", "paid", "packed", "shipped", "delivered", "cancelled")
TERMINAL_STATES = frozenset({"delivered", "cancelled"})
TRANSITIONS: dict[str, frozenset[str]] = {
    "pending": frozenset({"paid", "cancelled"}),
    "paid": frozenset({"packed", "cancelled"}),
    "packed": frozenset({"shipped"}),
    "shipped": frozenset({"delivered"}),
    "delivered": frozenset(),
    "cancelled": frozenset(),
}


class Order(Base):
    __tablename__ = "orders"

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    customer_id: Mapped[str] = mapped_column(ForeignKey("customers.id"), index=True)
    state: Mapped[str] = mapped_column(String(16), default="pending", index=True)
    ship_to: Mapped[dict] = mapped_column(JSON)
    courier_note: Mapped[str | None] = mapped_column(String(280), nullable=True)
    subtotal_cents: Mapped[int] = mapped_column(Integer)
    discount_cents: Mapped[int] = mapped_column(Integer, default=0)
    shipping_cents: Mapped[int] = mapped_column(Integer)
    total_cents: Mapped[int] = mapped_column(Integer)
    transitions: Mapped[list] = mapped_column(JSON, default=list)
    created_at: Mapped[datetime] = mapped_column(DateTime)
    updated_at: Mapped[datetime] = mapped_column(DateTime)

    lines: Mapped[list["Line"]] = relationship(back_populates="order", cascade="all, delete-orphan")


class Line(Base):
    __tablename__ = "order_lines"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    order_id: Mapped[str] = mapped_column(ForeignKey("orders.id"), index=True)
    sku: Mapped[str] = mapped_column(String(32))
    quantity: Mapped[int] = mapped_column(Integer)
    unit_cents: Mapped[int] = mapped_column(Integer)
    weight_grams: Mapped[int] = mapped_column(Integer)

    order: Mapped[Order] = relationship(back_populates="lines")
