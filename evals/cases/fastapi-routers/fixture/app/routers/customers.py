from fastapi import APIRouter, Depends, Response, status
from sqlalchemy.orm import Session

from ..deps import Authed, Db
from ..repositories.customers import CustomerRepository
from ..schemas.customers import CustomerIn, CustomerOut
from ..services.customers import CustomerService

router = APIRouter(prefix="/customers", tags=["customers"], dependencies=[Authed])


def service(db: Session = Db) -> CustomerService:
    return CustomerService(CustomerRepository(db))


def out(customer) -> CustomerOut:
    return CustomerOut(
        id=customer.id,
        email=customer.email,
        name=customer.name,
        tier=customer.tier,
        created_at=customer.created_at.isoformat(),
    )


@router.post("", response_model=CustomerOut, status_code=status.HTTP_201_CREATED)
def register(request: CustomerIn, response: Response, customers: CustomerService = Depends(service)) -> CustomerOut:
    customer = customers.register(request)
    response.headers["location"] = f"/api/v1/customers/{customer.id}"
    return out(customer)


@router.get("/{customer_id}", response_model=CustomerOut)
async def get_customer(customer_id: str, customers: CustomerService = Depends(service)) -> CustomerOut:
    return out(await customers.get(customer_id))


@router.post("/{customer_id}/tier/{tier}", response_model=CustomerOut)
async def promote(customer_id: str, tier: str, customers: CustomerService = Depends(service)) -> CustomerOut:
    return out(await customers.promote(customer_id, tier))
