from sqlalchemy import select
from sqlalchemy.orm import Session

from ..models import Customer


class CustomerRepository:
    def __init__(self, db: Session) -> None:
        self.db = db

    def find(self, customer_id: str) -> Customer | None:
        return self.db.get(Customer, customer_id)

    def by_email(self, email: str) -> Customer | None:
        return self.db.scalar(select(Customer).where(Customer.email == email.lower()))

    def insert(self, customer: Customer) -> Customer:
        self.db.add(customer)
        self.db.flush()
        return customer
