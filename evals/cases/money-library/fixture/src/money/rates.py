import json
from decimal import Decimal
from functools import lru_cache
from pathlib import Path

from .errors import UnknownRate
from .money import Currency, Money, Rounding

RATES_FILE = Path(__file__).parent / "data/rates.json"


@lru_cache(maxsize=1)
def table() -> dict[str, Decimal]:
    document = json.loads(RATES_FILE.read_text(encoding="utf-8"))
    return {code: Decimal(value) for code, value in document["rates"].items()}


def rate(source: Currency, target: Currency) -> Decimal:
    rates = table()
    if source.value not in rates:
        raise UnknownRate(source.value)
    if target.value not in rates:
        raise UnknownRate(target.value)
    return rates[target.value] / rates[source.value]


def convert(amount: Money, target: Currency, rounding: Rounding = Rounding.HALF_EVEN) -> Money:
    """The amount in the target currency at the table's rate; the same currency converts to itself."""
    if amount.currency is target:
        return amount
    factor = rate(amount.currency, target)
    return Money.from_decimal(amount.to_decimal() * factor, target, rounding)
