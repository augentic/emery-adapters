import { z } from "zod";
import { DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE } from "@app/config";

export const MAX_LINES = 50;
export const MAX_LINE_QUANTITY = 999;
export const MAX_COURIER_NOTE = 200;
export const NZ_POSTCODE = /^\d{4}$/;
export const SKU_PATTERN = /^[A-Z]{2,4}-\d{3,6}$/;

export const addressSchema = z.object({
  line1: z.string().min(1).max(120),
  line2: z.string().max(120).optional(),
  suburb: z.string().min(1).max(60),
  city: z.string().min(1).max(60),
  postcode: z.string().regex(NZ_POSTCODE, "a New Zealand postcode is four digits"),
});

export const lineSchema = z.object({
  sku: z.string().regex(SKU_PATTERN),
  quantity: z.number().int().min(1).max(MAX_LINE_QUANTITY),
});

export const placeOrderSchema = z.object({
  customerId: z.string().uuid(),
  lines: z.array(lineSchema).min(1).max(MAX_LINES),
  shipTo: addressSchema,
  courierNote: z.string().max(MAX_COURIER_NOTE).optional(),
});

export const amendLinesSchema = z.object({
  lines: z.array(lineSchema).min(1).max(MAX_LINES),
});

export const cancelSchema = z.object({
  reason: z.string().min(1).max(500),
});

export const listQuerySchema = z.object({
  customerId: z.string().uuid().optional(),
  state: z.enum(["pending", "paid", "shipped", "cancelled"]).optional(),
  page: z.coerce.number().int().min(1).default(1),
  pageSize: z.coerce.number().int().min(1).max(MAX_PAGE_SIZE).default(DEFAULT_PAGE_SIZE),
});

export type PlaceOrder = z.infer<typeof placeOrderSchema>;
export type AmendLines = z.infer<typeof amendLinesSchema>;
export type ListQuery = z.infer<typeof listQuerySchema>;
