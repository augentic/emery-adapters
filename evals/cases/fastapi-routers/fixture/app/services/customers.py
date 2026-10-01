import logging
from datetime import UTC, datetime
from uuid import uuid4

from ..cache import cache
from ..lib.errors import ConflictError, NotFoundError
from ..models import TIERS, Customer
from ..repositories.customers import CustomerRepository
from ..schemas.customers import CustomerIn
from ..settings import settings

log = logging.getLogger("shopapi.customers")


class CustomerService:
    def __init__(self, repository: CustomerRepository) -> None:
        self.repository = repository

    async def get(self, customer_id: str) -> Customer:
        cached = await cache.get(f"customer:{customer_id}")
        if cached is not None:
            return Customer(**cached)
        customer = self.repository.find(customer_id)
        if customer is None:
            raise NotFoundError("customer", customer_id)
        await cache.set(f"customer:{customer_id}", _dump(customer), settings.cache_ttl_s)
        return customer

    def register(self, request: CustomerIn) -> Customer:
        email = str(request.email).lower()
        if self.repository.by_email(email) is not None:
            raise ConflictError("registered", f"{email} is already registered")
        customer = Customer(id=str(uuid4()), email=email, name=request.name, tier=request.tier, created_at=datetime.now(UTC))
        self.repository.insert(customer)
        log.info("customer %s registered as %s", customer.id, customer.tier)
        return customer

    async def promote(self, customer_id: str, tier: str) -> Customer:
        if tier not in TIERS:
            raise ConflictError(tier, f"{tier} is not a tier")
        customer = await self.get(customer_id)
        if TIERS.index(tier) <= TIERS.index(customer.tier):
            raise ConflictError(customer.tier, f"customer {customer_id} is already {customer.tier}")
        customer.tier = tier
        self.repository.insert(customer)
        await cache.invalidate(f"customer:{customer_id}")
        return customer


def _dump(customer: Customer) -> dict:
    return {
        "id": customer.id,
        "email": customer.email,
        "name": customer.name,
        "tier": customer.tier,
        "created_at": customer.created_at,
    }
