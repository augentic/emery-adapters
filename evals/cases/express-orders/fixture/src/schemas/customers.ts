import { z } from "zod";

export const MAX_NAME = 80;

export const createCustomerSchema = z.object({
  email: z.string().email().max(254),
  name: z.string().min(1).max(MAX_NAME),
  tier: z.enum(["standard", "trade", "vip"]).default("standard"),
});

export const updateTierSchema = z.object({
  tier: z.enum(["standard", "trade", "vip"]),
});

export type CreateCustomer = z.infer<typeof createCustomerSchema>;
