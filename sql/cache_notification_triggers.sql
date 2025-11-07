-- SQL Trigger Examples for Cache Notifications
-- 
-- This file contains example PostgreSQL triggers and functions to send
-- cache invalidation notifications via LISTEN/NOTIFY when data changes occur.
--
-- The notifications use a single channel 'cache_invalidation' and include
-- table name, action type, and the full row data in JSON format.

-- =====================================================================
-- Generic Notification Function
-- =====================================================================
-- This function can be reused for any table by attaching it to triggers
-- It sends a notification with the table name, action, and row data

CREATE OR REPLACE FUNCTION notify_cache_change()
RETURNS TRIGGER AS $$
DECLARE
    notification json;
    payload text;
BEGIN
    -- Build the notification payload
    IF (TG_OP = 'DELETE') THEN
        notification = json_build_object(
            'table', TG_TABLE_NAME,
            'action', 'delete',
            'id', OLD.id
        );
    ELSE
        -- For INSERT and UPDATE, include the full row data
        notification = json_build_object(
            'table', TG_TABLE_NAME,
            'action', lower(TG_OP),
            'id', NEW.id,
            'data', row_to_json(NEW)
        );
    END IF;

    -- Convert to text and send notification
    payload = notification::text;
    PERFORM pg_notify('cache_invalidation', payload);

    -- Return the appropriate row
    IF (TG_OP = 'DELETE') THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- =====================================================================
-- Example: Users Table
-- =====================================================================

-- Create the users table (example schema)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Create trigger for users table
CREATE TRIGGER users_cache_notify
    AFTER INSERT OR UPDATE OR DELETE ON users
    FOR EACH ROW
    EXECUTE FUNCTION notify_cache_change();

-- =====================================================================
-- Example: Products Table
-- =====================================================================

-- Create the products table (example schema)
CREATE TABLE IF NOT EXISTS products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    product_name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Create trigger for products table
CREATE TRIGGER products_cache_notify
    AFTER INSERT OR UPDATE OR DELETE ON products
    FOR EACH ROW
    EXECUTE FUNCTION notify_cache_change();

-- =====================================================================
-- Test the Notification System
-- =====================================================================

-- First, in a separate session, listen for notifications:
-- LISTEN cache_invalidation;

-- Then run these test queries to see the notifications:

-- Test INSERT
-- INSERT INTO users (username, email) 
-- VALUES ('alice', 'alice@example.com');

-- Test UPDATE
-- UPDATE users SET email = 'alice.updated@example.com' 
-- WHERE username = 'alice';

-- Test DELETE
-- DELETE FROM users WHERE username = 'alice';

-- Example notification payloads you'll receive:
-- 
-- INSERT:
-- {
--   "table": "users",
--   "action": "insert",
--   "id": "550e8400-e29b-41d4-a716-446655440000",
--   "data": {
--     "id": "550e8400-e29b-41d4-a716-446655440000",
--     "username": "alice",
--     "email": "alice@example.com",
--     "created_at": "2024-01-01T12:00:00",
--     "updated_at": "2024-01-01T12:00:00"
--   }
-- }
--
-- UPDATE:
-- {
--   "table": "users",
--   "action": "update",
--   "id": "550e8400-e29b-41d4-a716-446655440000",
--   "data": {
--     "id": "550e8400-e29b-41d4-a716-446655440000",
--     "username": "alice",
--     "email": "alice.updated@example.com",
--     "created_at": "2024-01-01T12:00:00",
--     "updated_at": "2024-01-01T12:05:00"
--   }
-- }
--
-- DELETE:
-- {
--   "table": "users",
--   "action": "delete",
--   "id": "550e8400-e29b-41d4-a716-446655440000"
-- }

-- =====================================================================
-- Cleanup (if needed)
-- =====================================================================

-- DROP TRIGGER IF EXISTS users_cache_notify ON users;
-- DROP TRIGGER IF EXISTS products_cache_notify ON products;
-- DROP FUNCTION IF EXISTS notify_cache_change();
-- DROP TABLE IF EXISTS products;
-- DROP TABLE IF EXISTS users;