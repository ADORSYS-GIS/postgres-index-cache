-- SQL Trigger Examples for Cache Notifications
-- 
-- This file contains example PostgreSQL triggers and table schemas to demonstrate
-- cache invalidation notifications via LISTEN/NOTIFY when data changes occur.
--
-- These examples use the notify_cache_change() function defined in sql/cache_notification_triggers.sql

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

-- Create the user_index_cache table
CREATE TABLE IF NOT EXISTS user_index_cache (
    id UUID PRIMARY KEY,
    username_hash BIGINT NOT NULL,
    email_hash BIGINT NOT NULL
);

-- Create trigger for user_index_cache table
DROP TRIGGER IF EXISTS user_index_cache_notify ON user_index_cache;
CREATE TRIGGER user_index_cache_notify
    AFTER INSERT OR UPDATE OR DELETE ON user_index_cache
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

-- Create the product_index_cache table
CREATE TABLE IF NOT EXISTS product_index_cache (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    product_name_hash BIGINT NOT NULL
);

-- Create trigger for product_index_cache table
DROP TRIGGER IF EXISTS product_index_cache_notify ON product_index_cache;
CREATE TRIGGER product_index_cache_notify
    AFTER INSERT OR UPDATE OR DELETE ON product_index_cache
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

-- DROP TRIGGER IF EXISTS user_index_cache_notify ON user_index_cache;
-- DROP TRIGGER IF EXISTS product_index_cache_notify ON product_index_cache;
-- DROP FUNCTION IF EXISTS notify_cache_change();
-- DROP TABLE IF EXISTS product_index_cache;
-- DROP TABLE IF EXISTS user_index_cache;
-- DROP TABLE IF EXISTS products;
-- DROP TABLE IF EXISTS users;