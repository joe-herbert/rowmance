-- SQLite seed schema: customers (parent) + orders (child via FK)
-- Covers: INTEGER PRIMARY KEY AUTOINCREMENT, TEXT UUID, TEXT, BOOLEAN
--         (stored as 0/1), NUMERIC, REAL, DATE, TIMESTAMP, JSON-as-text,
--         BLOB, CHECK constraints, FK (enforced only if the client turns
--         on "PRAGMA foreign_keys = ON" per-connection)

DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;

CREATE TABLE customers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid_col      TEXT NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    full_name     TEXT NOT NULL,
    email         TEXT NOT NULL UNIQUE,
    bio           TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT 1,
    credit_limit  NUMERIC(10,2) DEFAULT 0,
    rating        REAL,
    signup_date   DATE NOT NULL DEFAULT (date('now')),
    created_at    TIMESTAMP NOT NULL DEFAULT (datetime('now')),
    metadata      TEXT,   -- JSON stored as text; SQLite has no native JSON type
    avatar        BLOB,
    status        TEXT NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active', 'inactive', 'pending'))
);

CREATE TABLE orders (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id      INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    order_number     TEXT NOT NULL UNIQUE,
    amount           NUMERIC(12,2) NOT NULL,
    quantity         INTEGER NOT NULL DEFAULT 1,
    discount         REAL DEFAULT 0,
    is_paid          BOOLEAN NOT NULL DEFAULT 0,
    order_date       DATE NOT NULL DEFAULT (date('now')),
    placed_at        TIMESTAMP NOT NULL DEFAULT (datetime('now')),
    notes            TEXT,
    shipping_address TEXT,
    receipt          BLOB,
    order_status     TEXT NOT NULL DEFAULT 'pending'
                        CHECK (order_status IN ('pending', 'shipped', 'delivered', 'cancelled'))
);

INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Ada Lovelace', 'ada@example.com', 'Mathematician and writer.', 1, 5000.00, 4.8, '{"tier":"gold"}', 'active'),
('Alan Turing',  'alan@example.com', 'Computer scientist.',       1, 3000.00, 4.6, '{"tier":"silver"}', 'active'),
('Grace Hopper', 'grace@example.com', NULL,                       0, 0,       NULL, NULL, 'inactive');

INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1001', 249.99, 2, 10.00, 1, 'Gift wrap requested', '{"city":"London","zip":"E1 6AN"}',    'delivered'),
(1, 'ORD-1002', 59.50,  1, 0,     0, NULL,                  '{"city":"London","zip":"E1 6AN"}',    'pending'),
(2, 'ORD-1003', 899.00, 5, 50.00, 1, 'Rush order',          '{"city":"Manchester","zip":"M1 1AE"}', 'shipped');
