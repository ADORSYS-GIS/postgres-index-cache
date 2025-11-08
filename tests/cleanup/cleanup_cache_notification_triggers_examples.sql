-- Cleanup: Cache Notification Triggers Examples
-- Description: Removes all artifacts created by tests/migrations/cache_notification_triggers_examples.sql

-- Drop triggers first
DROP TRIGGER IF EXISTS user_index_cache_notify ON user_index_cache;
DROP TRIGGER IF EXISTS product_index_cache_notify ON product_index_cache;

-- Drop tables (CASCADE will also drop dependent objects)
DROP TABLE IF EXISTS product_index_cache CASCADE;
DROP TABLE IF EXISTS products CASCADE;
DROP TABLE IF EXISTS user_index_cache CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Drop the notification function (CASCADE will also drop any remaining triggers using it)
DROP FUNCTION IF EXISTS notify_cache_change() CASCADE;