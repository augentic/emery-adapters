import { Request, Response, Router } from "express";
import { Producer } from "kafkajs";
import { Config } from "./config";

export interface OrderLine {
    sku: string;
    quantity: number;
}

export interface Order {
    id: string;
    customerId: string;
    lines: OrderLine[];
}

export interface OrderPlaced {
    type: "order-placed";
    order: Order;
    placedAt: string;
}

export function ordersRouter(producer: Producer): Router {
    const router = Router();

    router.post("/orders", async (req: Request, res: Response) => {
        const order = req.body as Order;
        if (!order.lines || order.lines.length === 0) {
            res.status(400).json({ error: "An order carries at least one line" });
            return;
        }
        if (order.lines.some((line) => line.quantity < 1)) {
            res.status(400).json({ error: "Each line quantity is at least 1" });
            return;
        }

        const event: OrderPlaced = { type: "order-placed", order, placedAt: new Date().toISOString() };
        await producer.send({
            topic: Config.kafka.topic,
            messages: [{ key: order.id, value: JSON.stringify(event) }],
        });

        res.status(202).json({ id: order.id });
    });

    return router;
}
