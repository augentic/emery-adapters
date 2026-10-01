import httpx

from ..config import INVOICE_CONCURRENCY, MAIL_API_TOKEN, MAIL_API_URL, log
from ..lib.db import mark_invoice
from ..queues import app

REMINDER_SUBJECT = "Invoice overdue"
MAX_REMINDER_AMOUNT_CENTS = 5_000_000
MAIL_TIMEOUT_S = 10


@app.task(name="invoices.remind", bind=True, max_retries=3, rate_limit=f"{INVOICE_CONCURRENCY}/s")
def send_reminder(self, invoice_id: str, customer_id: str, amount_cents: int) -> None:
    if amount_cents > MAX_REMINDER_AMOUNT_CENTS:
        log.warning("invoice %s above the reminder ceiling; routing to collections", invoice_id)
        mark_invoice(invoice_id, "void")
        return
    try:
        response = httpx.post(
            f"{MAIL_API_URL}/send",
            headers={"authorization": f"Bearer {MAIL_API_TOKEN}"},
            json={
                "to": customer_id,
                "subject": REMINDER_SUBJECT,
                "template": "invoice-overdue",
                "variables": {"invoiceId": invoice_id, "amount": f"{amount_cents / 100:.2f}"},
            },
            timeout=MAIL_TIMEOUT_S,
        )
    except httpx.TransportError as error:
        raise self.retry(exc=error)
    if response.status_code >= 300:
        raise RuntimeError(f"mail api answered {response.status_code}")
    log.info("reminder sent for invoice %s", invoice_id)


@app.task(name="invoices.void")
def void_invoice(invoice_id: str) -> None:
    mark_invoice(invoice_id, "void")
    log.info("invoice %s voided", invoice_id)
