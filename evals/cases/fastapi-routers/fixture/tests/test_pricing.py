import pytest

from app.repositories.catalogue import Priced
from app.services.pricing import discount, shipping, totals

LINES = [Priced("A-1", 2, 1_000, 200), Priced("B-2", 1, 3_000, 900)]


def test_a_standard_customer_gets_no_discount():
    assert discount(LINES, "standard") == 0


def test_a_gold_customer_gets_five_percent_off():
    assert discount(LINES, "gold") == 250


def test_two_dozen_units_earn_the_bulk_discount_when_it_beats_the_tier():
    bulk = [Priced("A-1", 24, 100, 10)]
    assert discount(bulk, "silver") == 72


def test_shipping_is_waived_at_the_threshold():
    assert shipping(15_000, 850, "1010") == 0


def test_a_rural_postcode_adds_the_surcharge():
    assert shipping(1_000, 850, "0140") == 850 + 650


@pytest.mark.parametrize("tier", ["standard", "silver", "gold"])
def test_totals_sum_the_parts(tier):
    result = totals(LINES, tier, 850, "1010")
    assert result["total_cents"] == result["subtotal_cents"] - result["discount_cents"] + result["shipping_cents"]
