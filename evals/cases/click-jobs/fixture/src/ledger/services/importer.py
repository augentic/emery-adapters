from dataclasses import asdict, dataclass
from pathlib import Path

from ..config import log
from ..lib.csv import parse_ledger_csv
from ..lib.db import insert_entries


@dataclass(frozen=True)
class ImportSummary:
    file: str
    parsed: int
    inserted: int
    rejected: int
    dry_run: bool

    def as_dict(self) -> dict:
        return asdict(self)


def import_ledger(file: str, dry_run: bool) -> ImportSummary:
    text = Path(file).read_text(encoding="utf-8")
    parsed = parse_ledger_csv(text)
    for error in parsed.errors:
        log.warning("%s:%s %s", file, error.line, error.message)
    inserted = 0 if dry_run else insert_entries(parsed.entries)
    summary = ImportSummary(
        file=file,
        parsed=len(parsed.entries),
        inserted=inserted,
        rejected=len(parsed.errors),
        dry_run=dry_run,
    )
    log.info("import %s: %s", "previewed" if dry_run else "committed", summary.as_dict())
    return summary
