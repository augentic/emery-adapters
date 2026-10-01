from dataclasses import dataclass

import httpx

from ..config import BANK_FEED_TOKEN, BANK_FEED_URL, FEED_TIMEOUT_S, RECONCILE_TOLERANCE_CENTS, log
from ..lib.db import LedgerEntry, entries_between, insert_entries


@dataclass(frozen=True)
class BankTransaction:
    id: str
    account: str
    date: str
    amount_cents: int
    narrative: str


@dataclass(frozen=True)
class Reconciliation:
    start: str
    end: str
    matched: int
    unmatched_ledger: list[LedgerEntry]
    unmatched_bank: list[BankTransaction]
    imported: int


def fetch_bank_transactions(start: str, end: str) -> list[BankTransaction]:
    url = f"{BANK_FEED_URL}/transactions"
    response = httpx.get(
        url,
        params={"from": start, "to": end},
        headers={"authorization": f"Bearer {BANK_FEED_TOKEN}"},
        timeout=FEED_TIMEOUT_S,
    )
    if response.status_code != 200:
        raise RuntimeError(f"bank feed answered {response.status_code} for {url}")
    return [BankTransaction(**item) for item in response.json()["transactions"]]


def same_movement(entry: LedgerEntry, transaction: BankTransaction) -> bool:
    return (
        entry.account == transaction.account
        and entry.posted_on == transaction.date
        and abs(entry.amount_cents - transaction.amount_cents) <= RECONCILE_TOLERANCE_CENTS
    )


def reconcile(start: str, end: str, import_unmatched: bool) -> Reconciliation:
    ledger = entries_between(start, end)
    unmatched_bank = fetch_bank_transactions(start, end)
    unmatched_ledger: list[LedgerEntry] = []
    matched = 0

    for entry in ledger:
        index = next((i for i, tx in enumerate(unmatched_bank) if same_movement(entry, tx)), None)
        if index is None:
            unmatched_ledger.append(entry)
            continue
        del unmatched_bank[index]
        matched += 1

    imported = 0
    if import_unmatched and unmatched_bank:
        imported = insert_entries(
            [
                LedgerEntry(
                    id=f"bank-{tx.id}",
                    account=tx.account,
                    posted_on=tx.date,
                    amount_cents=tx.amount_cents,
                    reference=tx.narrative[:140],
                    source="bank",
                )
                for tx in unmatched_bank
            ]
        )

    log.info(
        "reconciled %s..%s: matched=%s unmatched_ledger=%s unmatched_bank=%s imported=%s",
        start,
        end,
        matched,
        len(unmatched_ledger),
        len(unmatched_bank),
        imported,
    )
    return Reconciliation(start, end, matched, unmatched_ledger, unmatched_bank, imported)
