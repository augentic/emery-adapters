import type { OrderState } from "@app/domain/order";

export class ValidationError extends Error {}

export class UnauthorizedError extends Error {}

export class NotFoundError extends Error {
  constructor(readonly entity: string, readonly id: string) {
    super(`no ${entity} ${id}`);
  }
}

export class ConflictError extends Error {
  constructor(readonly state: OrderState, message: string) {
    super(message);
  }
}

export class UpstreamError extends Error {
  constructor(readonly upstream: string, message: string) {
    super(message);
  }
}
