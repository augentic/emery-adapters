from celery import Celery

from .config import INVOICE_ATTEMPTS, REDIS_URL

INVOICE_QUEUE = "invoices"
INVOICE_BACKOFF_S = 30

app = Celery("ledger", broker=REDIS_URL, backend=REDIS_URL, include=["ledger.workers.invoices"])
app.conf.update(
    task_default_queue=INVOICE_QUEUE,
    task_acks_late=True,
    task_annotations={"*": {"max_retries": INVOICE_ATTEMPTS, "default_retry_delay": INVOICE_BACKOFF_S}},
    result_expires=3600,
)
