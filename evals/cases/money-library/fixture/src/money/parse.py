import csv
import io
from decimal import Decimal, InvalidOperation

from .errors import ParseError
from .money import Currency, Money, Rounding

REQUIRED_COLUMNS = ("amount", "currency")
MAX_ROWS = 10_000


def parse_csv(text: str, rounding: Rounding = Rounding.HALF_EVEN) -> list[Money]:
    """Read `amount,currency` rows into Money, in order; the first bad row is the error."""
    reader = csv.DictReader(io.StringIO(text), skipinitialspace=True)
    header = reader.fieldnames or []
    missing = [column for column in REQUIRED_COLUMNS if column not in header]
    if missing:
        raise ParseError(1, f"missing columns: {', '.join(missing)}")
    amounts: list[Money] = []
    for index, row in enumerate(reader):
        line = index + 2
        if line - 1 > MAX_ROWS:
            raise ParseError(line, f"more than {MAX_ROWS} rows")
        code = (row.get("currency") or "").strip().upper()
        try:
            currency = Currency(code)
        except ValueError:
            raise ParseError(line, f"unknown currency {code!r}") from None
        raw = (row.get("amount") or "").strip().replace(",", "")
        try:
            value = Decimal(raw)
        except InvalidOperation:
            raise ParseError(line, f"amount {raw!r} is not a number") from None
        amounts.append(Money.from_decimal(value, currency, rounding))
    return amounts

