"""Seed a development database with a handful of ledger entries and invoices."""

import sys

from ledger.lib.db import LedgerEntry, insert_entries, pool

ENTRIES = [
    LedgerEntry("seed-1", "4000", "2026-01-05", 125_00, "Opening balance", "manual"),
    LedgerEntry("seed-2", "4000-01", "2026-01-06", -40_00, "Office supplies", "manual"),
    LedgerEntry("seed-3", "5100", "2026-01-07", 990_00, "Invoice INV-1001", "manual"),
]


def seed() -> int:
    inserted = insert_entries(ENTRIES)
    with pool.connection() as conn:
        conn.execute(
            "INSERT INTO invoices (id, customer_id, amount_cents, due_on, status) VALUES (%s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
            ("INV-1001", "cus_42", 990_00, "2026-01-21", "issued"),
        )
        conn.commit()
    return inserted


if __name__ == "__main__":
    print(f"seeded {seed()} entries")
    sys.exit(0)
