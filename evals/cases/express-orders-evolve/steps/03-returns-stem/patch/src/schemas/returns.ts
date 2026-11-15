import { z } from "zod";

export const createReturnSchema = z.object({
  orderId: z.string().uuid(),
  reason: z.string().min(1).max(500),
});

export type CreateReturn = z.infer<typeof createReturnSchema>;
