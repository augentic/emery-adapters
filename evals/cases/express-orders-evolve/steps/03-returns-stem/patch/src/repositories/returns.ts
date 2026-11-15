import type { Pool } from "pg";
import type { OrderReturn } from "@app/domain/returns";

interface ReturnRow {
  id: string;
  order_id: string;
  reason: string;
  created_at: Date;
}

function toOrderReturn(row: ReturnRow): OrderReturn {
  return {
    id: row.id,
    orderId: row.order_id,
    reason: row.reason,
    createdAt: row.created_at,
  };
}

export class ReturnRepository {
  constructor(private readonly pool: Pool) {}

  async find(id: string): Promise<OrderReturn | undefined> {
    const { rows } = await this.pool.query<ReturnRow>("SELECT * FROM returns WHERE id = $1", [id]);
    return rows[0] === undefined ? undefined : toOrderReturn(rows[0]);
  }

  async insert(entry: OrderReturn): Promise<void> {
    await this.pool.query(
      "INSERT INTO returns (id, order_id, reason, created_at) VALUES ($1, $2, $3, $4)",
      [entry.id, entry.orderId, entry.reason, entry.createdAt],
    );
  }
}
