import type Redis from "ioredis";
import { CACHE_TTL_SECONDS, logger } from "@app/config";

const KEY_PREFIX = "orders-api:";

export class Cache {
  constructor(private readonly redis: Redis) {}

  async get<T>(key: string): Promise<T | undefined> {
    try {
      const raw = await this.redis.get(KEY_PREFIX + key);
      return raw === null ? undefined : (JSON.parse(raw) as T);
    } catch (error) {
      logger.warn({ err: error, key }, "cache read failed; treating as miss");
      return undefined;
    }
  }

  async set(key: string, value: unknown, ttlSeconds: number = CACHE_TTL_SECONDS): Promise<void> {
    try {
      await this.redis.set(KEY_PREFIX + key, JSON.stringify(value), "EX", ttlSeconds);
    } catch (error) {
      logger.warn({ err: error, key }, "cache write failed; continuing");
    }
  }

  async invalidate(...keys: string[]): Promise<void> {
    if (keys.length === 0) {
      return;
    }
    try {
      await this.redis.del(...keys.map((key) => KEY_PREFIX + key));
    } catch (error) {
      logger.warn({ err: error, keys }, "cache invalidation failed; continuing");
    }
  }
}
