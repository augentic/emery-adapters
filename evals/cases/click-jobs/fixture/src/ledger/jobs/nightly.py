from datetime import date, datetime, timedelta

from apscheduler.schedulers.background import BackgroundScheduler

from ..config import NIGHTLY_HOUR, TIMEZONE, log
from ..lib.db import invoices_due_before, mark_invoice
from ..services.reconcile import reconcile
from ..workers.invoices import send_reminder

OVERDUE_REMINDER_PRIORITY = 2

scheduler = BackgroundScheduler(timezone=TIMEZONE)


def nightly(now: datetime | None = None) -> None:
    today = (now or datetime.now()).date()
    yesterday = today - timedelta(days=1)

    result = reconcile(yesterday.isoformat(), today.isoformat(), True)
    if result.unmatched_ledger:
        log.warning("%s ledger entries from %s without a bank movement", len(result.unmatched_ledger), yesterday)

    overdue = invoices_due_before(today.isoformat())
    for invoice in overdue:
        mark_invoice(invoice.id, "overdue")
        send_reminder.apply_async(
            kwargs={"invoice_id": invoice.id, "customer_id": invoice.customer_id, "amount_cents": invoice.amount_cents},
            task_id=f"remind-{invoice.id}-{today.isoformat()}",
            priority=OVERDUE_REMINDER_PRIORITY,
        )
    log.info("nightly run complete: overdue=%s matched=%s", len(overdue), result.matched)


def schedule() -> BackgroundScheduler:
    scheduler.add_job(nightly, "cron", hour=NIGHTLY_HOUR, id="nightly", misfire_grace_time=3600)
    scheduler.add_listener(_failed, mask=0b0010_0000_0000)
    scheduler.start()
    log.info("nightly job scheduled at %02d:00 %s", NIGHTLY_HOUR, TIMEZONE)
    return scheduler


def _failed(event) -> None:
    log.error("nightly run failed: %s", event.exception)


def is_weekend(day: date) -> bool:
    return day.weekday() >= 5
