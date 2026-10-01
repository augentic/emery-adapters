from dataclasses import dataclass

from sqlalchemy import Integer, String, select
from sqlalchemy.orm import Mapped, Session, mapped_column

from ..db import Base


class Product(Base):
    __tablename__ = "products"

    sku: Mapped[str] = mapped_column(String(32), primary_key=True)
    name: Mapped[str] = mapped_column(String(120))
    unit_cents: Mapped[int] = mapped_column(Integer)
    weight_grams: Mapped[int] = mapped_column(Integer)
    stock: Mapped[int] = mapped_column(Integer, default=0)


@dataclass(frozen=True)
class Priced:
    sku: str
    quantity: int
    unit_cents: int
    weight_grams: int


class Catalogue:
    def __init__(self, db: Session) -> None:
        self.db = db

    def products(self, skus: list[str]) -> dict[str, Product]:
        rows = self.db.scalars(select(Product).where(Product.sku.in_(skus))).all()
        return {row.sku: row for row in rows}
