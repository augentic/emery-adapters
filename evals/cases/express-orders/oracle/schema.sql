-- The relational schema the Express service's repositories read and write,
-- as their SQL implies it: the catalogue the lines are priced from, the
-- customers, and the orders with their lines, address, and transitions held
-- as JSON. The oracle applies it before each scenario, so a port keeps the
-- tables and their columns while its statements stay its own.

CREATE TABLE catalogue (
    sku          TEXT PRIMARY KEY,
    description  TEXT NOT NULL,
    unit_cents   INTEGER NOT NULL,
    weight_grams INTEGER NOT NULL,
    active       BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE customers (
    id         UUID PRIMARY KEY,
    email      TEXT NOT NULL,
    name       TEXT NOT NULL,
    tier       TEXT NOT NULL CHECK (tier IN ('standard', 'trade', 'vip')),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE orders (
    id             UUID PRIMARY KEY,
    customer_id    UUID NOT NULL REFERENCES customers (id),
    lines          JSONB NOT NULL,
    ship_to        JSONB NOT NULL,
    courier_note   TEXT,
    state          TEXT NOT NULL CHECK (state IN ('pending', 'paid', 'shipped', 'cancelled')),
    subtotal_cents INTEGER NOT NULL,
    discount_cents INTEGER NOT NULL,
    shipping_cents INTEGER NOT NULL,
    gst_cents      INTEGER NOT NULL,
    total_cents    INTEGER NOT NULL,
    transitions    JSONB NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL
);
