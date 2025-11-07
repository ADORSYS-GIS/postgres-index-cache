-- Production Cache Notification Function
--
-- This file contains the production PostgreSQL trigger function to send
-- cache invalidation notifications via LISTEN/NOTIFY when data changes occur.
--
-- The notifications use a single channel 'cache_invalidation' and include
-- table name, action type, and the full row data in JSON format.

-- =====================================================================
-- Generic Notification Function
-- =====================================================================
-- This function can be reused for any table by attaching it to triggers
-- It sends a notification with the table name, action, and row data

DROP FUNCTION IF EXISTS notify_cache_change() CASCADE;

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