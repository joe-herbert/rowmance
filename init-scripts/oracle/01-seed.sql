-- Oracle seed schema: customers (parent) + orders (child via FK)
-- Runs as SYS during container startup, so we switch schema context to
-- APP_USER (created via the APP_USER env var) so the tables land in a
-- normal, queryable user schema instead of under SYS.
-- Covers: IDENTITY PK, VARCHAR2 GUID, VARCHAR2, CLOB, NUMBER(1) boolean,
--         NUMBER(p,s), FLOAT, DATE, TIMESTAMP, JSON, BLOB, CHECK, FK w/ CASCADE

ALTER SESSION SET CURRENT_SCHEMA = APPUSER;

BEGIN
  EXECUTE IMMEDIATE 'DROP TABLE orders CASCADE CONSTRAINTS';
EXCEPTION WHEN OTHERS THEN NULL;
END;
/

BEGIN
  EXECUTE IMMEDIATE 'DROP TABLE customers CASCADE CONSTRAINTS';
EXCEPTION WHEN OTHERS THEN NULL;
END;
/

CREATE TABLE customers (
    id            NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    uuid_col      VARCHAR2(36) DEFAULT SYS_GUID() NOT NULL,
    full_name     VARCHAR2(120) NOT NULL,
    email         VARCHAR2(200) NOT NULL UNIQUE,
    bio           CLOB,
    is_active     NUMBER(1) DEFAULT 1 NOT NULL CHECK (is_active IN (0, 1)),
    credit_limit  NUMBER(10,2) DEFAULT 0,
    rating        FLOAT,
    signup_date   DATE DEFAULT SYSDATE NOT NULL,
    created_at    TIMESTAMP DEFAULT SYSTIMESTAMP NOT NULL,
    metadata      JSON,
    avatar        BLOB,
    status        VARCHAR2(20) DEFAULT 'active' NOT NULL
                    CHECK (status IN ('active', 'inactive', 'pending'))
);

CREATE TABLE orders (
    id                NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    customer_id       NUMBER NOT NULL,
    order_number      VARCHAR2(40) NOT NULL UNIQUE,
    amount            NUMBER(12,2) NOT NULL,
    quantity          NUMBER(10) DEFAULT 1 NOT NULL,
    discount          FLOAT DEFAULT 0,
    is_paid           NUMBER(1) DEFAULT 0 NOT NULL CHECK (is_paid IN (0, 1)),
    order_date        DATE DEFAULT SYSDATE NOT NULL,
    placed_at         TIMESTAMP DEFAULT SYSTIMESTAMP NOT NULL,
    notes             CLOB,
    shipping_address  JSON,
    receipt           BLOB,
    order_status      VARCHAR2(20) DEFAULT 'pending' NOT NULL
                        CHECK (order_status IN ('pending', 'shipped', 'delivered', 'cancelled')),
    CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Ada Lovelace', 'ada@example.com', 'Mathematician and writer.', 1, 5000.00, 4.8, '{"tier":"gold"}', 'active');
INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Alan Turing', 'alan@example.com', 'Computer scientist.', 1, 3000.00, 4.6, '{"tier":"silver"}', 'active');
INSERT INTO customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Grace Hopper', 'grace@example.com', NULL, 0, 0, NULL, NULL, 'inactive');

INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1001', 249.99, 2, 10.00, 1, 'Gift wrap requested', '{"city":"London","zip":"E1 6AN"}', 'delivered');
INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1002', 59.50, 1, 0, 0, NULL, '{"city":"London","zip":"E1 6AN"}', 'pending');
INSERT INTO orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(2, 'ORD-1003', 899.00, 5, 50.00, 1, 'Rush order', '{"city":"Manchester","zip":"M1 1AE"}', 'shipped');

COMMIT;
