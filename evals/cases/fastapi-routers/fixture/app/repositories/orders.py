from sqlalchemy import func, select
from sqlalchemy.orm import Session, selectinload

from ..models import Order


class OrderRepository:
    def __init__(self, db: Session) -> None:
        self.db = db

    def find(self, order_id: str) -> Order | None:
        return self.db.scalar(select(Order).options(selectinload(Order.lines)).where(Order.id == order_id))

    def list(self, *, customer_id: str | None, state: str | None, page: int, size: int) -> tuple[list[Order], int]:
        query = select(Order).options(selectinload(Order.lines))
        if customer_id is not None:
            query = query.where(Order.customer_id == customer_id)
        if state is not None:
            query = query.where(Order.state == state)
        total = self.db.scalar(select(func.count()).select_from(query.subquery())) or 0
        rows = self.db.scalars(query.order_by(Order.created_at.desc()).offset((page - 1) * size).limit(size)).all()
        return list(rows), total

    def insert(self, order: Order) -> Order:
        self.db.add(order)
        self.db.flush()
        return order

    def update(self, order: Order) -> Order:
        self.db.add(order)
        self.db.flush()
        return order
