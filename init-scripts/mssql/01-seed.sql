-- SQL Server seed schema: customers (parent) + orders (child via FK)
-- Covers: IDENTITY PK, UNIQUEIDENTIFIER, NVARCHAR, NVARCHAR(MAX), BIT,
--         DECIMAL, FLOAT, DATE, DATETIME2, JSON-as-text, VARBINARY,
--         CHECK constraints, FK w/ CASCADE

USE testdb;
GO

IF OBJECT_ID('dbo.orders', 'U') IS NOT NULL DROP TABLE dbo.orders;
IF OBJECT_ID('dbo.customers', 'U') IS NOT NULL DROP TABLE dbo.customers;
GO

CREATE TABLE dbo.customers (
    id            INT IDENTITY(1,1) PRIMARY KEY,
    uuid_col      UNIQUEIDENTIFIER NOT NULL DEFAULT NEWID(),
    full_name     NVARCHAR(120) NOT NULL,
    email         NVARCHAR(200) NOT NULL UNIQUE,
    bio           NVARCHAR(MAX) NULL,
    is_active     BIT NOT NULL DEFAULT 1,
    credit_limit  DECIMAL(10,2) DEFAULT 0,
    rating        FLOAT NULL,
    signup_date   DATE NOT NULL DEFAULT CAST(GETDATE() AS DATE),
    created_at    DATETIME2 NOT NULL DEFAULT SYSDATETIME(),
    metadata      NVARCHAR(MAX) NULL,   -- JSON stored as text; validate with ISJSON() if desired
    avatar        VARBINARY(MAX) NULL,
    status        NVARCHAR(20) NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active', 'inactive', 'pending'))
);
GO

CREATE TABLE dbo.orders (
    id                INT IDENTITY(1,1) PRIMARY KEY,
    customer_id       INT NOT NULL,
    order_number      NVARCHAR(40) NOT NULL UNIQUE,
    amount            DECIMAL(12,2) NOT NULL,
    quantity          INT NOT NULL DEFAULT 1,
    discount          FLOAT DEFAULT 0,
    is_paid           BIT NOT NULL DEFAULT 0,
    order_date        DATE NOT NULL DEFAULT CAST(GETDATE() AS DATE),
    placed_at         DATETIME2 NOT NULL DEFAULT SYSDATETIME(),
    notes             NVARCHAR(MAX) NULL,
    shipping_address  NVARCHAR(MAX) NULL,
    receipt           VARBINARY(MAX) NULL,
    order_status      NVARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (order_status IN ('pending', 'shipped', 'delivered', 'cancelled')),
    CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES dbo.customers(id) ON DELETE CASCADE
);
GO

INSERT INTO dbo.customers (full_name, email, bio, is_active, credit_limit, rating, metadata, status) VALUES
('Ada Lovelace', 'ada@example.com', 'Mathematician and writer.', 1, 5000.00, 4.8, '{"tier":"gold"}', 'active'),
('Alan Turing',  'alan@example.com', 'Computer scientist.',       1, 3000.00, 4.6, '{"tier":"silver"}', 'active'),
('Grace Hopper', 'grace@example.com', NULL,                       0, 0,       NULL, NULL, 'inactive');
GO

INSERT INTO dbo.orders (customer_id, order_number, amount, quantity, discount, is_paid, notes, shipping_address, order_status) VALUES
(1, 'ORD-1001', 249.99, 2, 10.00, 1, 'Gift wrap requested', '{"city":"London","zip":"E1 6AN"}',    'delivered'),
(1, 'ORD-1002', 59.50,  1, 0,     0, NULL,                  '{"city":"London","zip":"E1 6AN"}',    'pending'),
(2, 'ORD-1003', 899.00, 5, 50.00, 1, 'Rush order',          '{"city":"Manchester","zip":"M1 1AE"}', 'shipped');
GO
