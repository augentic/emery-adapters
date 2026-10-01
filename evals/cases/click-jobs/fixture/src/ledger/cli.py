import json
import signal
import sys
import time

import click

from .config import log
from .jobs.nightly import nightly, schedule
from .lib.db import pool
from .queues import app
from .services.importer import import_ledger
from .services.reconcile import reconcile


@click.group()
@click.version_option("0.9.1", prog_name="ledger")
def cli() -> None:
    """Ledger import, reconciliation, and invoice processing."""


@cli.command("import")
@click.argument("file", type=click.Path(exists=True, dir_okay=False))
@click.option("--dry-run", is_flag=True, default=False, help="parse and report without writing")
def import_command(file: str, dry_run: bool) -> None:
    """Import a ledger CSV export."""
    summary = import_ledger(file, dry_run)
    click.echo(json.dumps(summary.as_dict()))
    if summary.rejected > 0 and not dry_run:
        sys.exit(2)


@cli.command("reconcile")
@click.option("--from", "start", required=True, help="first posting date, inclusive")
@click.option("--to", "end", required=True, help="last posting date, exclusive")
@click.option("--import", "import_unmatched", is_flag=True, default=False, help="import unmatched bank movements")
def reconcile_command(start: str, end: str, import_unmatched: bool) -> None:
    """Match ledger entries against the bank feed for a date range."""
    result = reconcile(start, end, import_unmatched)
    click.echo(
        json.dumps(
            {
                "from": result.start,
                "to": result.end,
                "matched": result.matched,
                "unmatchedLedger": len(result.unmatched_ledger),
                "unmatchedBank": len(result.unmatched_bank),
                "imported": result.imported,
            }
        )
    )
    if result.unmatched_ledger or result.unmatched_bank:
        sys.exit(3)


@cli.command("nightly")
def nightly_command() -> None:
    """Run the nightly reconciliation and overdue sweep once, now."""
    nightly()


@cli.command("serve")
def serve() -> None:
    """Run the invoice worker and the nightly schedule until stopped."""
    scheduler = schedule()
    worker = app.Worker(concurrency=4, queues=["invoices"], loglevel="INFO")

    def stop(signum, _frame) -> None:
        log.info("stopping on %s", signal.Signals(signum).name)
        scheduler.shutdown(wait=False)
        worker.stop()
        pool.close()
        sys.exit(0)

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    log.info("ledger serving")
    worker.start()
    while True:
        time.sleep(60)


@cli.result_callback()
def close(_result, **_kwargs) -> None:
    if click.get_current_context().invoked_subcommand != "serve":
        pool.close()


def main() -> None:
    try:
        cli(standalone_mode=False)
    except click.ClickException as error:
        error.show()
        sys.exit(error.exit_code)
    except Exception as error:
        log.error("command failed: %s", error)
        sys.exit(1)


if __name__ == "__main__":
    main()
