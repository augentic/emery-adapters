class MoneyError(Exception):
    """The base of every error this package raises."""


class CurrencyMismatch(MoneyError):
    def __init__(self, left: str, right: str) -> None:
        super().__init__(f"cannot combine {left} with {right}")
        self.left = left
        self.right = right


class ParseError(MoneyError):
    def __init__(self, line: int, message: str) -> None:
        super().__init__(f"line {line}: {message}")
        self.line = line


class UnknownRate(MoneyError):
    def __init__(self, currency: str) -> None:
        super().__init__(f"no rate for {currency}")
        self.currency = currency
