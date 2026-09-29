import type { Pool, PoolClient } from "pg";
import type { Order, OrderLine, OrderState, Page, Transition } from "@app/domain/order";
import type { ListQuery } from "@app/schemas/orders";

interface OrderRow {
  id: string;
  customer_id: string;
  lines: OrderLine[];
  ship_to: Order["shipTo"];
  courier_note: string | null;
  state: OrderState;
  subtotal_cents: number;
  discount_cents: number;
  shipping_cents: number;
  gst_cents: number;
  total_cents: number;
  transitions: { state: OrderState; at: string; note?: string }[];
  created_at: Date;
  updated_at: Date;
}

export interface Catalogue {
  sku: string;
  description: string;
  unit_cents: number;
  weight_grams: number;
  active: boolean;
}

function toOrder(row: OrderRow): Order {
  return {
    id: row.id,
    customerId: row.customer_id,
    lines: row.lines,
    shipTo: row.ship_to,
    courierNote: row.courier_note ?? undefined,
    state: row.state,
    subtotalCents: row.subtotal_cents,
    discountCents: row.discount_cents,
    shippingCents: row.shipping_cents,
    gstCents: row.gst_cents,
    totalCents: row.total_cents,
    transitions: row.transitions.map((transition): Transition => ({ ...transition, at: new Date(transition.at) })),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

export class OrderRepository {
  constructor(private readonly pool: Pool) {}

  async catalogue(skus: readonly string[]): Promise<Catalogue[]> {
    const { rows } = await this.pool.query<Catalogue>(
      "SELECT sku, description, unit_cents, weight_grams, active FROM catalogue WHERE sku = ANY($1)",
      [skus],
    );
    return rows;
  }

  async find(id: string): Promise<Order | undefined> {
    const { rows } = await this.pool.query<OrderRow>("SELECT * FROM orders WHERE id = $1", [id]);
    return rows[0] === undefined ? undefined : toOrder(rows[0]);
  }

  async list(query: ListQuery): Promise<Page<Order>> {
    const conditions: string[] = [];
    const params: unknown[] = [];
    if (query.customerId !== undefined) {
      params.push(query.customerId);
      conditions.push(`customer_id = $${params.length}`);
    }
    if (query.state !== undefined) {
      params.push(query.state);
      conditions.push(`state = $${params.length}`);
    }
    const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";
    const offset = (query.page - 1) * query.pageSize;
    const [{ rows }, count] = await Promise.all([
      this.pool.query<OrderRow>(
        `SELECT * FROM orders ${where} ORDER BY created_at DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
        [...params, query.pageSize, offset],
      ),
      this.pool.query<{ total: string }>(`SELECT count(*)::text AS total FROM orders ${where}`, params),
    ]);
    return {
      items: rows.map(toOrder),
      total: Number(count.rows[0]?.total ?? 0),
      page: query.page,
      pageSize: query.pageSize,
    };
  }

  async insert(order: Order): Promise<void> {
    await this.pool.query(
      `INSERT INTO orders (id, customer_id, lines, ship_to, courier_note, state, subtotal_cents, discount_cents,
         shipping_cents, gst_cents, total_cents, transitions, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)`,
      [
        order.id,
        order.customerId,
        JSON.stringify(order.lines),
        JSON.stringify(order.shipTo),
        order.courierNote ?? null,
        order.state,
        order.subtotalCents,
        order.discountCents,
        order.shippingCents,
        order.gstCents,
        order.totalCents,
        JSON.stringify(order.transitions),
        order.createdAt,
        order.updatedAt,
      ],
    );
  }

  async update(order: Order): Promise<void> {
    const client: PoolClient = await this.pool.connect();
    try {
      await client.query("BEGIN");
      const { rowCount } = await client.query(
        `UPDATE orders SET lines = $2, state = $3, subtotal_cents = $4, discount_cents = $5, shipping_cents = $6,
           gst_cents = $7, total_cents = $8, transitions = $9, updated_at = $10
         WHERE id = $1 AND updated_at < $10`,
        [
          order.id,
          JSON.stringify(order.lines),
          order.state,
          order.subtotalCents,
          order.discountCents,
          order.shippingCents,
          order.gstCents,
          order.totalCents,
          JSON.stringify(order.transitions),
          order.updatedAt,
        ],
      );
      if (rowCount !== 1) {
        throw new Error(`order ${order.id} was modified concurrently`);
      }
      await client.query("COMMIT");
    } catch (error) {
      await client.query("ROLLBACK");
      throw error;
    } finally {
      client.release();
    }
  }
}
