import csv
import io
import re
from dataclasses import dataclass

from .db import LedgerEntry

MAX_ROWS = 50_000
REQUIRED_COLUMNS = ("id", "account", "posted_on", "amount", "reference")
ACCOUNT_CODE = re.compile(r"^\d{4}(-\d{2})?$")
ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
REFERENCE_LENGTH = 140


@dataclass(frozen=True)
class RowError:
    line: int
    message: str


@dataclass(frozen=True)
class ParsedImport:
    entries: list[LedgerEntry]
    errors: list[RowError]


def parse_ledger_csv(text: str) -> ParsedImport:
    reader = csv.DictReader(io.StringIO(text), skipinitialspace=True)
    records = list(reader)
    if len(records) > MAX_ROWS:
        raise ValueError(f"import has {len(records)} rows; the limit is {MAX_ROWS}")
    header = reader.fieldnames or []
    missing = [column for column in REQUIRED_COLUMNS if column not in header]
    if records and missing:
        raise ValueError(f"import is missing columns: {', '.join(missing)}")

    entries: list[LedgerEntry] = []
    errors: list[RowError] = []
    for index, record in enumerate(records):
        line = index + 2
        account = record["account"].strip()
        if not ACCOUNT_CODE.match(account):
            errors.append(RowError(line, f"account {account} is not a four-digit code"))
            continue
        posted_on = record["posted_on"].strip()
        if not ISO_DATE.match(posted_on):
            errors.append(RowError(line, f"posted_on {posted_on} is not an ISO date"))
            continue
        try:
            amount_cents = round(float(record["amount"]) * 100)
        except ValueError:
            amount_cents = 0
        if amount_cents == 0:
            errors.append(RowError(line, f"amount {record['amount']} is not a non-zero number"))
            continue
        entries.append(
            LedgerEntry(
                id=record["id"].strip(),
                account=account,
                posted_on=posted_on,
                amount_cents=amount_cents,
                reference=record["reference"].strip()[:REFERENCE_LENGTH],
                source="import",
            )
        )
    return ParsedImport(entries=entries, errors=errors)
