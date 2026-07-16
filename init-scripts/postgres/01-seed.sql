-- Postgres seed schema: customers (parent) + orders (child via FK)
-- Covers: SERIAL PK, UUID, VARCHAR, TEXT, BOOLEAN, NUMERIC, REAL,
--         DATE, TIMESTAMP, JSONB, BYTEA, CHECK constraints, FK w/ CASCADE

DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;

CREATE TABLE customers (
    id            SERIAL PRIMARY KEY,
    uuid_col      UUID NOT NULL DEFAULT gen_random_uuid(),
    full_name     VARCHAR(120) NOT NULL,
    email         VARCHAR(200) NOT NULL UNIQUE,
    bio           TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    credit_limit  NUMERIC(10,2) DEFAULT 0,
    rating        REAL,
    signup_date   DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    metadata      JSONB,
    avatar        BYTEA,
    status        VARCHAR(20) NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active', 'inactive', 'pending'))
);

CREATE TABLE orders (
    id               SERIAL PRIMARY KEY,
    customer_id      INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    order_number     VARCHAR(40) NOT NULL UNIQUE,
    amount           NUMERIC(12,2) NOT NULL,
    quantity         INTEGER NOT NULL DEFAULT 1,
    discount         REAL DEFAULT 0,
    is_paid          BOOLEAN NOT NULL DEFAULT FALSE,
    order_date       DATE NOT NULL DEFAULT CURRENT_DATE,
    placed_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    notes            TEXT,
    shipping_address JSONB,
    receipt          BYTEA,
    order_status     VARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (order_status IN ('pending', 'shipped', 'delivered', 'cancelled'))
);

INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, avatar, status) VALUES
('Ada Lovelace', 'ada@example.com', 'Mathematician and writer.', TRUE, 5000.00, 4.8, '{"tier":"gold"}', decode('89504e47', 'hex'), 'active'),
('Alan Turing',  'alan@example.com', 'Computer scientist.',       TRUE, 3000.00, 4.6, '{"tier":"silver"}', NULL, 'active'),
('Grace Hopper', 'grace@example.com', NULL,                       FALSE, 0,       NULL, NULL, NULL, 'inactive');

INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1001', 249.99, 2, 10.00, TRUE,  'Gift wrap requested', '{"city":"London","zip":"E1 6AN"}',    'delivered'),
(1, 'ORD-1002', 59.50,  1, 0,     FALSE, NULL,                  '{"city":"London","zip":"E1 6AN"}',    'pending'),
(2, 'ORD-1003', 899.00, 5, 50.00, TRUE,  'Rush order',          '{"city":"Manchester","zip":"M1 1AE"}', 'shipped');
