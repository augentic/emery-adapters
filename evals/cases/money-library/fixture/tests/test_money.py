import pytest

from money import Currency, CurrencyMismatch, Money, MoneyError, ParseError, Rounding, convert, parse_csv


class TestMoney:
    def test_adds_two_amounts_of_one_currency(self):
        assert Money(150, Currency.NZD).add(Money(250, Currency.NZD)) == Money(400, Currency.NZD)

    def test_refuses_to_add_across_currencies(self):
        with pytest.raises(CurrencyMismatch):
            Money(150, Currency.NZD).add(Money(250, Currency.AUD))

    def test_rounds_half_even_by_default(self):
        assert Money.from_decimal("1.005", Currency.NZD) == Money(100, Currency.NZD)
        assert Money.from_decimal("1.005", Currency.NZD, Rounding.HALF_UP) == Money(101, Currency.NZD)

    def test_yen_has_no_minor_units(self):
        assert str(Money(1200, Currency.JPY)) == "1200 JPY"

    def test_allocates_the_remainder_from_the_first_share(self):
        """Allocating 100 cents by 1:1:1 is 34, 33, 33."""
        shares = Money(100, Currency.USD).allocate([1, 1, 1])
        assert [share.amount for share in shares] == [34, 33, 33]

    def test_refuses_an_amount_beyond_the_maximum(self):
        with pytest.raises(MoneyError):
            Money(10**16, Currency.NZD)


class TestParseCsv:
    def test_reads_amounts_in_order(self):
        amounts = parse_csv("amount,currency\n1.50,nzd\n2,JPY\n")
        assert amounts == [Money(150, Currency.NZD), Money(2, Currency.JPY)]

    def test_requires_the_header(self):
        with pytest.raises(ParseError, match="missing columns"):
            parse_csv("value\n1.50\n")

    @pytest.mark.parametrize("row", ["1.50,XXX", "abc,NZD"])
    def test_names_the_first_bad_row(self, row):
        with pytest.raises(ParseError, match="line 2"):
            parse_csv(f"amount,currency\n{row}\n")


def test_converts_at_the_table_rate():
    assert convert(Money(10000, Currency.NZD), Currency.USD) == Money(6085, Currency.USD)

