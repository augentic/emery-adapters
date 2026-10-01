from dataclasses import dataclass
from typing import Literal

import psycopg
from psycopg_pool import ConnectionPool

from ..config import DATABASE_URL

Source = Literal["import", "manual", "bank"]
InvoiceStatus = Literal["draft", "issued", "paid", "overdue", "void"]


@dataclass(frozen=True)
class LedgerEntry:
    id: str
    account: str
    posted_on: str
    amount_cents: int
    reference: str
    source: Source


@dataclass(frozen=True)
class Invoice:
    id: str
    customer_id: str
    amount_cents: int
    due_on: str
    status: InvoiceStatus


pool = ConnectionPool(DATABASE_URL, min_size=1, max_size=5)


def insert_entries(entries: list[LedgerEntry]) -> int:
    if not entries:
        return 0
    inserted = 0
    with pool.connection() as conn:
        try:
            with conn.cursor() as cur:
                for entry in entries:
                    cur.execute(
                        """
                        INSERT INTO ledger (id, account, posted_on, amount_cents, reference, source)
                        VALUES (%s, %s, %s, %s, %s, %s)
                        ON CONFLICT (id) DO NOTHING
                        """,
                        (entry.id, entry.account, entry.posted_on, entry.amount_cents, entry.reference, entry.source),
                    )
                    inserted += cur.rowcount
            conn.commit()
        except psycopg.Error:
            conn.rollback()
            raise
    return inserted


def entries_between(start: str, end: str) -> list[LedgerEntry]:
    with pool.connection() as conn, conn.cursor() as cur:
        cur.execute(
            """
            SELECT id, account, posted_on, amount_cents, reference, source
            FROM ledger WHERE posted_on >= %s AND posted_on < %s ORDER BY posted_on
            """,
            (start, end),
        )
        return [LedgerEntry(*row) for row in cur.fetchall()]


def invoices_due_before(date: str) -> list[Invoice]:
    with pool.connection() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, customer_id, amount_cents, due_on, status FROM invoices WHERE status = 'issued' AND due_on < %s",
            (date,),
        )
        return [Invoice(*row) for row in cur.fetchall()]


def mark_invoice(invoice_id: str, status: InvoiceStatus) -> None:
    with pool.connection() as conn:
        conn.execute("UPDATE invoices SET status = %s, updated_at = now() WHERE id = %s", (status, invoice_id))
        conn.commit()
