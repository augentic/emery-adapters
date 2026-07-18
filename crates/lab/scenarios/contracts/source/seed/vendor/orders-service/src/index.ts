import express from "express";
import { createOrder, getOrder } from "./orders/handlers";

const app = express();
app.use(express.json());

app.post("/orders", createOrder);
app.get("/orders/:orderId", getOrder);

app.listen(3000);
