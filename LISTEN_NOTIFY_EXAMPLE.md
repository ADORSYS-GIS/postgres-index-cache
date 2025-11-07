# PostgreSQL LISTEN/NOTIFY Example for Index Cache

This document demonstrates how to use the PostgreSQL LISTEN/NOTIFY functionality with the `postgres-index-cache` library to automatically update caches when database changes occur.

## Overview

The single-channel approach uses one PostgreSQL notification channel (`cache_invalidation`) to handle notifications from all tables. Each notification includes the table name, action type (insert/update/delete), and the affected row data.

## Architecture

```
┌─────────────────┐
│   Application   │
│   Node 1        │──┐
└─────────────────┘  │
                     │    ┌──────────────────────────┐
┌─────────────────┐  │    │    PostgreSQL Server     │
│   Application   │──┼───►│  NOTIFY via triggers     │
│   Node 2        │  │    │  channel: cache_invalid  │
└─────────────────┘  │    └──────────────────────────┘
                     │              │
┌─────────────────┐  │              │ LISTEN
│   Application   │──┘              │
│   Node N        │                 ▼
└─────────────────┘    ┌────────────────────────┐
         │             │  Notification Handler  │
         │             │  - Parses payload      │
         │             │  - Routes to cache     │
         ▼             └────────────────────────┘
┌─────────────────┐              │
│  IndexCache     │◄─────────────┘
│  (in-memory)    │
└─────────────────┘
```

## Step 1: Set Up PostgreSQL Triggers

First, create the notification function and triggers in PostgreSQL. See [`sql/cache_notification_triggers.sql`](sql/cache_notification_triggers.sql) for the complete SQL setup.

```sql
-- Create the generic notification function
CREATE OR REPLACE FUNCTION notify_cache_change()
RETURNS TRIGGER AS $$
DECLARE
    notification json;
    payload text;
BEGIN
    IF (TG_OP = 'DELETE') THEN
        notification = json_build_object(
            'table', TG_TABLE_NAME,
            'action', 'delete',
            'id', OLD.id
        );
    ELSE
        notification = json_build_object(
            'table', TG_TABLE_NAME,
            'action', lower(TG_OP),
            'id', NEW.id,
            'data', row_to_json(NEW)
        );
    END IF;

    payload = notification::text;
    PERFORM pg_notify('cache_invalidation', payload);

    IF (TG_OP = 'DELETE') THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Attach trigger to your table
CREATE TRIGGER users_cache_notify
    AFTER INSERT OR UPDATE OR DELETE ON users
    FOR EACH ROW
    EXECUTE FUNCTION notify_cache_change();
```

## Step 2: Define Your Cache Models

Your cache models must implement the required traits and be serializable:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;
use postgres_index_cache::{HasPrimaryKey, Indexable};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIndexCache {
    pub id: Uuid,
    pub username_hash: i64,
    pub email_hash: i64,
}

impl HasPrimaryKey for UserIndexCache {
    fn primary_key(&self) -> Uuid {
        self.id
    }
}

impl Indexable for UserIndexCache {
    fn i64_keys(&self) -> HashMap<String, Option<i64>> {
        let mut map = HashMap::new();
        map.insert("username_hash".to_string(), Some(self.username_hash));
        map.insert("email_hash".to_string(), Some(self.email_hash));
        map
    }

    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>> {
        HashMap::new()
    }
}
```

## Step 3: Set Up the Listener

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use postgres_index_cache::{
    IdxModelCache, 
    CacheNotificationListener, 
    IndexCacheHandler
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize your caches
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![])?));
    
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![])?));
    
    // Create handlers for each cache
    let user_handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    let product_handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    // Create listener and register handlers
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(user_handler);
    listener.register_handler(product_handler);
    
    // Your application logic here
    // The listener will process notifications via process_notification()
    
    Ok(())
}
```

## Step 4: Integrate with Your Notification Loop

The library provides a [`process_notification()`](src/listener.rs:144) method that you call from your own notification polling loop. Here's how to integrate with `tokio-postgres`:

```rust
use tokio_postgres::{NoTls, AsyncMessage};
use futures::StreamExt;

async fn start_notification_loop(
    listener: Arc<CacheNotificationListener>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Connect to PostgreSQL
    let (client, mut connection) = tokio_postgres::connect(
        "host=localhost user=postgres dbname=mydb",
        NoTls,
    ).await?;
    
    // Execute LISTEN command
    client.execute(
        &format!("LISTEN {}", listener.channel()), 
        &[]
    ).await?;
    
    // Spawn task to handle connection
    let listener_clone = listener.clone();
    tokio::spawn(async move {
        while let Some(message) = connection.next().await {
            if let Ok(AsyncMessage::Notification(notif)) = message {
                // Process the notification through the listener
                listener_clone.process_notification(notif.payload()).await;
            }
        }
    });
    
    Ok(())
}
```

## Step 5: Use Your Cached Data

Now your caches will automatically stay in sync with database changes:

```rust
// Read from cache
let cache = user_cache.read();

// Find by primary key
if let Some(user) = cache.get_by_primary(&user_id) {
    println!("Found user: {:?}", user);
}

// Find by secondary index
if let Some(user_ids) = cache.get_by_i64_index("username_hash", &hash) {
    for id in user_ids {
        if let Some(user) = cache.get_by_primary(id) {
            println!("User: {:?}", user);
        }
    }
}
```

## Notification Payload Format

Notifications follow this JSON structure:

### INSERT/UPDATE
```json
{
  "table": "users",
  "action": "insert",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username_hash": 123456789,
    "email_hash": 987654321
  }
}
```

### DELETE
```json
{
  "table": "users",
  "action": "delete",
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## Benefits

1. **Decoupled Architecture**: Nodes don't need to know about each other
2. **Single Channel**: All tables use the same notification channel
3. **Real-time Updates**: Caches update immediately when data changes
4. **Automatic Routing**: Notifications are automatically routed to the correct cache
5. **Action-specific Handling**: Insert, update, and delete operations are handled appropriately

## Testing

See [`tests/listener_test.rs`](tests/listener_test.rs) for comprehensive test examples demonstrating:
- Insert notifications
- Update notifications
- Delete notifications
- Multi-table handling
- Unknown table handling

Run tests with:
```bash
cargo test --test listener_test
```

## Advanced Usage

### Custom Channel Name

```rust
let listener = CacheNotificationListener::with_channel("my_custom_channel".to_string());
```

### Custom Notification Handler

You can implement your own notification handler:

```rust
use async_trait::async_trait;
use postgres_index_cache::{CacheNotification, CacheNotificationHandler};

struct MyCustomHandler {
    // Your fields
}

#[async_trait]
impl CacheNotificationHandler for MyCustomHandler {
    async fn handle_notification(&self, notification: CacheNotification) {
        // Your custom logic
    }
    
    fn table_name(&self) -> &str {
        "my_table"
    }
}
```

## See Also

- [Main README](README.md) - General library documentation
- [SQL Triggers](sql/cache_notification_triggers.sql) - Complete SQL setup examples
- [Tests](tests/listener_test.rs) - Working code examples