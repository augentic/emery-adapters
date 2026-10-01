import sqlite3
import threading
from dataclasses import dataclass, field
from typing import Literal

Status = Literal["pending", "paid", "shipped", "cancelled"]


@dataclass
class Order:
    id: int
    customer: str
    lines: list[dict] = field(default_factory=list)
    status: Status = "pending"
    total_cents: int = 0
    shipping_cents: int = 0

    def as_dict(self) -> dict:
        return {
            "id": self.id,
            "customer": self.customer,
            "lines": self.lines,
            "status": self.status,
            "total_cents": self.total_cents,
            "shipping_cents": self.shipping_cents,
        }


class OrderRepository:
    """Orders in a SQLite file, one connection per thread."""

    def __init__(self, path: str = "shop.db") -> None:
        self.path = path
        self.local = threading.local()

    def connection(self) -> sqlite3.Connection:
        conn = getattr(self.local, "conn", None)
        if conn is None:
            conn = sqlite3.connect(self.path)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS orders (id INTEGER PRIMARY KEY AUTOINCREMENT, customer TEXT, lines TEXT, status TEXT, total_cents INTEGER, shipping_cents INTEGER)"
            )
            self.local.conn = conn
        return conn

    def insert(self, order: Order) -> Order:
        conn = self.connection()
        cursor = conn.execute(
            "INSERT INTO orders (customer, lines, status, total_cents, shipping_cents) VALUES (?, ?, ?, ?, ?)",
            (order.customer, repr(order.lines), order.status, order.total_cents, order.shipping_cents),
        )
        conn.commit()
        order.id = int(cursor.lastrowid or 0)
        return order

    def find(self, order_id: int) -> Order | None:
        row = self.connection().execute(
            "SELECT id, customer, lines, status, total_cents, shipping_cents FROM orders WHERE id = ?", (order_id,)
        ).fetchone()
        if row is None:
            return None
        return Order(row[0], row[1], eval(row[2]), row[3], row[4], row[5])

    def for_customer(self, customer: str, limit: int) -> list[Order]:
        rows = self.connection().execute(
            "SELECT id, customer, lines, status, total_cents, shipping_cents FROM orders WHERE customer = ? ORDER BY id DESC LIMIT ?",
            (customer, limit),
        ).fetchall()
        return [Order(row[0], row[1], eval(row[2]), row[3], row[4], row[5]) for row in rows]

    def set_status(self, order_id: int, status: Status) -> None:
        conn = self.connection()
        conn.execute("UPDATE orders SET status = ? WHERE id = ?", (status, order_id))
        conn.commit()


repository = OrderRepository()
