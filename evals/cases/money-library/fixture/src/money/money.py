from __future__ import annotations

from dataclasses import dataclass
from decimal import ROUND_DOWN, ROUND_HALF_EVEN, ROUND_HALF_UP, Decimal
from enum import Enum

from .errors import CurrencyMismatch, MoneyError

MAX_AMOUNT = 10**15
ALLOCATION_RATIOS_MAX = 100


class Currency(str, Enum):
    NZD = "NZD"
    AUD = "AUD"
    USD = "USD"
    EUR = "EUR"
    JPY = "JPY"

    @property
    def minor_units(self) -> int:
        return 0 if self is Currency.JPY else 2


class Rounding(Enum):
    HALF_EVEN = ROUND_HALF_EVEN
    HALF_UP = ROUND_HALF_UP
    DOWN = ROUND_DOWN


@dataclass(frozen=True, order=True)
class Money:
    """An amount in a currency's minor units: cents for NZD, yen for JPY."""

    amount: int
    currency: Currency

    def __post_init__(self) -> None:
        if abs(self.amount) > MAX_AMOUNT:
            raise MoneyError(f"amount {self.amount} is beyond {MAX_AMOUNT}")

    @classmethod
    def from_decimal(cls, value: Decimal | str, currency: Currency, rounding: Rounding = Rounding.HALF_EVEN) -> Money:
        scale = Decimal(10) ** currency.minor_units
        minor = (Decimal(value) * scale).quantize(Decimal(1), rounding=rounding.value)
        return cls(int(minor), currency)

    def to_decimal(self) -> Decimal:
        return Decimal(self.amount) / (Decimal(10) ** self.currency.minor_units)

    def _same(self, other: Money) -> None:
        if self.currency is not other.currency:
            raise CurrencyMismatch(self.currency.value, other.currency.value)

    def add(self, other: Money) -> Money:
        self._same(other)
        return Money(self.amount + other.amount, self.currency)

    def subtract(self, other: Money) -> Money:
        self._same(other)
        return Money(self.amount - other.amount, self.currency)

    def multiply(self, factor: Decimal | int | str, rounding: Rounding = Rounding.HALF_EVEN) -> Money:
        product = (Decimal(self.amount) * Decimal(factor)).quantize(Decimal(1), rounding=rounding.value)
        return Money(int(product), self.currency)

    def allocate(self, ratios: list[int]) -> list[Money]:
        """Split the amount by the ratios, handing the remainder one unit at a time from the first share."""
        if not ratios or any(ratio < 0 for ratio in ratios) or sum(ratios) == 0:
            raise MoneyError("ratios must be non-negative and sum above zero")
        if len(ratios) > ALLOCATION_RATIOS_MAX:
            raise MoneyError(f"at most {ALLOCATION_RATIOS_MAX} shares")
        total = sum(ratios)
        shares = [self.amount * ratio // total for ratio in ratios]
        remainder = self.amount - sum(shares)
        for index in range(remainder):
            shares[index % len(shares)] += 1
        return [Money(share, self.currency) for share in shares]

    def is_zero(self) -> bool:
        return self.amount == 0

    def __str__(self) -> str:
        return f"{self.to_decimal():.{self.currency.minor_units}f} {self.currency.value}"

