import type { Pool } from "pg";
import type { Customer, CustomerTier } from "@app/domain/order";

interface CustomerRow {
  id: string;
  email: string;
  name: string;
  tier: CustomerTier;
  created_at: Date;
}

function toCustomer(row: CustomerRow): Customer {
  return { id: row.id, email: row.email, name: row.name, tier: row.tier, createdAt: row.created_at };
}

export class CustomerRepository {
  constructor(private readonly pool: Pool) {}

  async find(id: string): Promise<Customer | undefined> {
    const { rows } = await this.pool.query<CustomerRow>("SELECT * FROM customers WHERE id = $1", [id]);
    return rows[0] === undefined ? undefined : toCustomer(rows[0]);
  }

  async findByEmail(email: string): Promise<Customer | undefined> {
    const { rows } = await this.pool.query<CustomerRow>("SELECT * FROM customers WHERE lower(email) = lower($1)", [email]);
    return rows[0] === undefined ? undefined : toCustomer(rows[0]);
  }

  async insert(customer: Customer): Promise<void> {
    await this.pool.query(
      "INSERT INTO customers (id, email, name, tier, created_at) VALUES ($1, $2, $3, $4, $5)",
      [customer.id, customer.email, customer.name, customer.tier, customer.createdAt],
    );
  }

  async setTier(id: string, tier: CustomerTier): Promise<boolean> {
    const { rowCount } = await this.pool.query("UPDATE customers SET tier = $2 WHERE id = $1", [id, tier]);
    return rowCount === 1;
  }
}
