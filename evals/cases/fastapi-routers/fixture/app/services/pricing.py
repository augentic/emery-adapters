import json
from pathlib import Path

from ..data.postcodes import RURAL_SURCHARGE_CENTS, zone_for
from ..repositories.catalogue import Priced

RATES = json.loads((Path(__file__).parent.parent / "data" / "rates.json").read_text(encoding="utf-8"))

TIER_DISCOUNT_BPS: dict[str, int] = RATES["tier_discount_bps"]
FREE_SHIPPING_THRESHOLD_CENTS: int = RATES["free_shipping_threshold_cents"]
BULK_DISCOUNT_BPS: int = RATES["bulk_discount_bps"]
BULK_UNITS: int = RATES["bulk_units"]


def subtotal(lines: list[Priced]) -> int:
    return sum(line.quantity * line.unit_cents for line in lines)


def weight(lines: list[Priced]) -> int:
    return sum(line.quantity * line.weight_grams for line in lines)


def discount(lines: list[Priced], tier: str) -> int:
    gross = subtotal(lines)
    bps = TIER_DISCOUNT_BPS.get(tier, 0)
    if sum(line.quantity for line in lines) >= BULK_UNITS:
        bps = max(bps, BULK_DISCOUNT_BPS)
    return gross * bps // 10_000


def shipping(subtotal_cents: int, quote_cents: int, postcode: str) -> int:
    if subtotal_cents >= FREE_SHIPPING_THRESHOLD_CENTS:
        return 0
    zone = zone_for(postcode)
    surcharge = RURAL_SURCHARGE_CENTS if zone is not None and zone.rural else 0
    return quote_cents + surcharge


def totals(lines: list[Priced], tier: str, quote_cents: int, postcode: str) -> dict[str, int]:
    gross = subtotal(lines)
    off = discount(lines, tier)
    ship = shipping(gross - off, quote_cents, postcode)
    return {
        "subtotal_cents": gross,
        "discount_cents": off,
        "shipping_cents": ship,
        "total_cents": gross - off + ship,
    }
