from datetime import datetime

from sqlalchemy import DateTime, String
from sqlalchemy.orm import Mapped, mapped_column

from ..db import Base

TIERS = ("standard", "silver", "gold")


class Customer(Base):
    __tablename__ = "customers"

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    email: Mapped[str] = mapped_column(String(254), unique=True, index=True)
    name: Mapped[str] = mapped_column(String(120))
    tier: Mapped[str] = mapped_column(String(16), default="standard")
    created_at: Mapped[datetime] = mapped_column(DateTime)
