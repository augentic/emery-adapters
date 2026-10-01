import logging
import os

DATABASE_URL = os.environ.get("DATABASE_URL", "postgres://ledger:ledger@localhost:5432/ledger")
REDIS_URL = os.environ.get("REDIS_URL", "redis://localhost:6379/0")
BANK_FEED_URL = os.environ.get("BANK_FEED_URL", "https://feeds.bank.example/v1")
BANK_FEED_TOKEN = os.environ.get("BANK_FEED_TOKEN", "")
NIGHTLY_HOUR = int(os.environ.get("NIGHTLY_HOUR", "2"))
TIMEZONE = os.environ.get("TZ", "Pacific/Auckland")
INVOICE_CONCURRENCY = int(os.environ.get("INVOICE_CONCURRENCY", "4"))
INVOICE_ATTEMPTS = int(os.environ.get("INVOICE_ATTEMPTS", "5"))
RECONCILE_TOLERANCE_CENTS = int(os.environ.get("RECONCILE_TOLERANCE_CENTS", "100"))
FEED_TIMEOUT_S = float(os.environ.get("FEED_TIMEOUT_S", "15"))
MAIL_API_URL = os.environ.get("MAIL_API_URL", "https://mail.example/v1")
MAIL_API_TOKEN = os.environ.get("MAIL_API_TOKEN", "")

logging.basicConfig(level=os.environ.get("LOG_LEVEL", "INFO"), format="%(asctime)s %(name)s %(message)s")
log = logging.getLogger("ledger-tool")
