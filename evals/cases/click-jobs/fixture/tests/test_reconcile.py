from ledger.lib.db import LedgerEntry
from ledger.services.reconcile import BankTransaction, same_movement


def entry(amount_cents: int) -> LedgerEntry:
    return LedgerEntry("e1", "4000", "2026-01-05", amount_cents, "Coffee", "import")


def movement(amount_cents: int) -> BankTransaction:
    return BankTransaction("b1", "4000", "2026-01-05", amount_cents, "COFFEE CO")


class TestSameMovement:
    def test_matches_within_the_tolerance(self):
        """An entry matches a movement on account and date when the amounts are within the tolerance."""
        assert same_movement(entry(1250), movement(1300))

    def test_does_not_match_past_the_tolerance(self):
        assert not same_movement(entry(1250), movement(1400))

    def test_does_not_match_another_account(self):
        other = BankTransaction("b1", "4100", "2026-01-05", 1250, "COFFEE CO")
        assert not same_movement(entry(1250), other)
