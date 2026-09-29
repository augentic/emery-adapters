import { randomUUID } from "node:crypto";
import type { Customer, CustomerTier } from "@app/domain/order";
import { ConflictError, NotFoundError } from "@app/lib/errors";
import type { CustomerRepository } from "@app/repositories/customers";
import type { CreateCustomer } from "@app/schemas/customers";

export class CustomerService {
  constructor(private readonly repository: CustomerRepository) {}

  async get(id: string): Promise<Customer> {
    const customer = await this.repository.find(id);
    if (customer === undefined) {
      throw new NotFoundError("customer", id);
    }
    return customer;
  }

  async create(request: CreateCustomer): Promise<Customer> {
    const existing = await this.repository.findByEmail(request.email);
    if (existing !== undefined) {
      throw new ConflictError("pending", `a customer already uses ${request.email}`);
    }
    const customer: Customer = {
      id: randomUUID(),
      email: request.email.toLowerCase(),
      name: request.name.trim(),
      tier: request.tier,
      createdAt: new Date(),
    };
    await this.repository.insert(customer);
    return customer;
  }

  async setTier(id: string, tier: CustomerTier): Promise<Customer> {
    const updated = await this.repository.setTier(id, tier);
    if (!updated) {
      throw new NotFoundError("customer", id);
    }
    return this.get(id);
  }
}
