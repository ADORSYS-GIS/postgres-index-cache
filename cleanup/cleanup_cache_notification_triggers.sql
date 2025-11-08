-- Cleanup: Cache Notification Triggers
-- Description: Removes all artifacts created by sql/cache_notification_triggers.sql

-- Drop the notification function (CASCADE will also drop any triggers using it)
DROP FUNCTION IF EXISTS notify_cache_change() CASCADE;