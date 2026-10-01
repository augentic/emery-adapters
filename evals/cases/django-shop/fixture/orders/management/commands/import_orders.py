import csv
from collections import defaultdict
from pathlib import Path

from django.contrib.auth import get_user_model
from django.core.management.base import BaseCommand, CommandError
from django.db import transaction

from orders.services import place

REQUIRED_COLUMNS = {"order_ref", "customer_email", "sku", "quantity", "unit_cents"}


class Command(BaseCommand):
    help = "Import orders from a CSV export, one row per line, grouped by order_ref"

    def add_arguments(self, parser) -> None:
        parser.add_argument("file", type=Path)
        parser.add_argument("--dry-run", action="store_true", help="validate and report without writing")

    def handle(self, *args, **options) -> None:
        path: Path = options["file"]
        if not path.is_file():
            raise CommandError(f"{path} is not a file")
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            missing = REQUIRED_COLUMNS - set(reader.fieldnames or [])
            if missing:
                raise CommandError(f"missing columns: {', '.join(sorted(missing))}")
            grouped: dict[tuple[str, str], list[dict]] = defaultdict(list)
            for row in reader:
                grouped[(row["order_ref"], row["customer_email"])].append(
                    {"sku": row["sku"], "quantity": int(row["quantity"]), "unit_cents": int(row["unit_cents"])}
                )

        User = get_user_model()
        placed = skipped = 0
        with transaction.atomic():
            for (ref, email), lines in grouped.items():
                customer = User.objects.filter(email=email).first()
                if customer is None:
                    self.stderr.write(f"{ref}: no customer {email}; skipped")
                    skipped += 1
                    continue
                if not options["dry_run"]:
                    place(customer, lines, note=f"imported {ref}")
                placed += 1
            if options["dry_run"]:
                transaction.set_rollback(True)
        verb = "would place" if options["dry_run"] else "placed"
        self.stdout.write(self.style.SUCCESS(f"{verb} {placed} orders, skipped {skipped}"))
