import pytest

from ledger.lib.csv import MAX_ROWS, parse_ledger_csv

HEADER = "id,account,posted_on,amount,reference\n"


def test_parses_a_well_formed_row():
    parsed = parse_ledger_csv(HEADER + "e1,4000,2026-01-05,12.50,Coffee\n")
    assert parsed.errors == []
    assert parsed.entries[0].amount_cents == 1250
    assert parsed.entries[0].source == "import"


def test_rejects_a_bad_account_code_with_its_line():
    parsed = parse_ledger_csv(HEADER + "e1,40,2026-01-05,12.50,Coffee\n")
    assert parsed.entries == []
    assert parsed.errors[0].line == 2


def test_rejects_a_non_iso_date():
    parsed = parse_ledger_csv(HEADER + "e1,4000,05/01/2026,12.50,Coffee\n")
    assert "ISO date" in parsed.errors[0].message


@pytest.mark.parametrize("amount", ["0", "0.00", "abc"])
def test_rejects_a_zero_or_non_numeric_amount(amount):
    parsed = parse_ledger_csv(HEADER + f"e1,4000,2026-01-05,{amount},Coffee\n")
    assert parsed.entries == []


def test_missing_columns_reject_the_import_whole():
    with pytest.raises(ValueError, match="missing columns"):
        parse_ledger_csv("id,account\ne1,4000\n")


def test_too_many_rows_reject_the_import_whole():
    rows = "".join(f"e{i},4000,2026-01-05,1.00,x\n" for i in range(MAX_ROWS + 1))
    with pytest.raises(ValueError, match="limit"):
        parse_ledger_csv(HEADER + rows)
