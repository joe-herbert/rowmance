-- MySQL seed schema: customers (parent) + orders (child via FK)
-- Covers: AUTO_INCREMENT PK, CHAR(36) UUID, VARCHAR, TEXT, BOOLEAN,
--         DECIMAL, FLOAT, DATE, TIMESTAMP, JSON, BLOB, CHECK, FK w/ CASCADE

DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;

CREATE TABLE customers (
    id           INT AUTO_INCREMENT PRIMARY KEY,
    uuid_col     CHAR(36) NOT NULL DEFAULT (UUID()),
    full_name    VARCHAR(120) NOT NULL,
    email        VARCHAR(200) NOT NULL UNIQUE,
    bio          TEXT,
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    credit_limit DECIMAL(10,2) DEFAULT 0,
    rating       FLOAT,
    signup_date  DATE NOT NULL DEFAULT (CURRENT_DATE),
    created_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    metadata     JSON,
    avatar       BLOB,
    status       VARCHAR(20) NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active', 'inactive', 'pending'))
) ENGINE=InnoDB;

CREATE TABLE orders (
    id               INT AUTO_INCREMENT PRIMARY KEY,
    customer_id      INT NOT NULL,
    order_number     VARCHAR(40) NOT NULL UNIQUE,
    amount           DECIMAL(12,2) NOT NULL,
    quantity         INT NOT NULL DEFAULT 1,
    discount         FLOAT DEFAULT 0,
    is_paid          BOOLEAN NOT NULL DEFAULT FALSE,
    order_date       DATE NOT NULL DEFAULT (CURRENT_DATE),
    placed_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    notes            TEXT,
    shipping_address JSON,
    receipt          BLOB,
    order_status     VARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (order_status IN ('pending', 'shipped', 'delivered', 'cancelled')),
    CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
) ENGINE=InnoDB;

INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Ada Lovelace', 'ada@example.com', 'Mathematician and writer.', TRUE, 5000.00, 4.8, '{"tier":"gold"}', 'active'),
('Alan Turing',  'alan@example.com', 'Computer scientist.',       TRUE, 3000.00, 4.6, '{"tier":"silver"}', 'active'),
('Grace Hopper', 'grace@example.com', NULL,                       FALSE, 0,       NULL, NULL, 'inactive');

INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1001', 249.99, 2, 10.00, TRUE,  'Gift wrap requested', '{"city":"London","zip":"E1 6AN"}',    'delivered'),
(1, 'ORD-1002', 59.50,  1, 0,     FALSE, NULL,                  '{"city":"London","zip":"E1 6AN"}',    'pending'),
(2, 'ORD-1003', 899.00, 5, 50.00, TRUE,  'Rush order',          '{"city":"Manchester","zip":"M1 1AE"}', 'shipped');
