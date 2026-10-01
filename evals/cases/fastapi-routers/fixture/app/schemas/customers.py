from typing import Literal

from pydantic import BaseModel, EmailStr, Field

Tier = Literal["standard", "silver", "gold"]


class CustomerIn(BaseModel):
    email: EmailStr
    name: str = Field(min_length=1, max_length=120)
    tier: Tier = "standard"


class CustomerOut(BaseModel):
    id: str
    email: str
    name: str
    tier: Tier
    created_at: str
