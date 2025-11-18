use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::index_cache::IdxModelCache;
use crate::traits::{HasPrimaryKey, Indexable};

/// The default channel name for cache notifications
pub const DEFAULT_CACHE_CHANNEL: &str = "cache_invalidation";

/// Notification payload structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheNotification {
    /// The table name that was modified
    pub table: String,
    /// The action performed: "insert", "update", or "delete"
    pub action: String,
    /// The primary key of the affected row
    pub id: Uuid,
    /// Optional: the full entity data for insert/update operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Handler trait for cache notifications
#[async_trait]
pub trait CacheNotificationHandler: Send + Sync {
    /// Handle a cache notification
    async fn handle_notification(&self, notification: CacheNotification);
    
    /// Get the table name this handler is responsible for
    fn table_name(&self) -> &str;
}

/// A notification handler for a specific IndexCache
pub struct IndexCacheHandler<T: HasPrimaryKey + Indexable + Clone + Send + Sync + 'static> {
    table_name: String,
    cache: Arc<RwLock<IdxModelCache<T>>>,
}

impl<T: HasPrimaryKey + Indexable + Clone + Send + Sync + 'static> IndexCacheHandler<T> {
    /// Create a new handler for the given cache
    pub fn new(table_name: String, cache: Arc<RwLock<IdxModelCache<T>>>) -> Self {
        Self { table_name, cache }
    }
}

#[async_trait]
impl<T: HasPrimaryKey + Indexable + Clone + Send + Sync + std::fmt::Debug + 'static> 
    CacheNotificationHandler for IndexCacheHandler<T> 
where
    T: for<'de> Deserialize<'de>,
{
    async fn handle_notification(&self, notification: CacheNotification) {
        debug!(
            "Handling notification for table '{}': action={}, id={}",
            notification.table, notification.action, notification.id
        );

        match notification.action.as_str() {
            "insert" | "update" => {
                if let Some(data) = notification.data {
                    match serde_json::from_value::<T>(data) {
                        Ok(item) => {
                            let mut cache = self.cache.write();
                            if notification.action == "insert" {
                                cache.add(item);
                                debug!("Added item {} to cache", notification.id);
                            } else {
                                cache.update(item);
                                debug!("Updated item {} in cache", notification.id);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to deserialize data for {}: {}",
                                notification.table, e
                            );
                        }
                    }
                } else {
                    warn!(
                        "No data provided for {} operation on table {}",
                        notification.action, notification.table
                    );
                }
            }
            "delete" => {
                let mut cache = self.cache.write();
                cache.remove(&notification.id);
                debug!("Removed item {} from cache", notification.id);
            }
            _ => {
                warn!("Unknown action '{}' for table '{}'", notification.action, notification.table);
            }
        }
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// Listener for PostgreSQL notifications that dispatches to registered cache handlers
pub struct CacheNotificationListener {
    handlers: HashMap<String, Arc<dyn CacheNotificationHandler>>,
    channel: String,
}

impl CacheNotificationListener {
    /// Create a new listener with the default channel
    pub fn new() -> Self {
        Self::with_channel(DEFAULT_CACHE_CHANNEL.to_string())
    }

    /// Create a new listener with a custom channel name
    pub fn with_channel(channel: String) -> Self {
        Self {
            handlers: HashMap::new(),
            channel,
        }
    }

    /// Register a handler for a specific table
    pub fn register_handler(&mut self, handler: Arc<dyn CacheNotificationHandler>) {
        let table_name = handler.table_name().to_string();
        debug!("Registering handler for table '{}'", table_name);
        self.handlers.insert(table_name, handler);
    }

    /// Process a single notification payload
    /// 
    /// This method can be called from your own notification polling loop.
    /// 
    /// # Example
    /// ```ignore
    /// // In your notification loop
    /// while let Some(notification) = get_notification().await {
    ///     listener.process_notification(&notification.payload()).await;
    /// }
    /// ```
    pub async fn process_notification(&self, payload: &str) {
        match serde_json::from_str::<CacheNotification>(payload) {
            Ok(cache_notif) => {
                if let Some(handler) = self.handlers.get(&cache_notif.table) {
                    handler.handle_notification(cache_notif).await;
                } else {
                    debug!(
                        "No handler registered for table '{}'",
                        cache_notif.table
                    );
                }
            }
            Err(e) => {
                error!("Failed to parse notification payload: {}", e);
                debug!("Payload was: {}", payload);
            }
        }
    }

    /// Get the channel name this listener is using
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Starts listening for notifications from PostgreSQL and processes them.
    ///
    /// This method will continuously listen for notifications on the configured
    /// channel and dispatch them to the appropriate handlers. It is designed to
    /// run in a background task.
    ///
    /// # Arguments
    ///
    /// * `pool` - A `PgPool` to connect to the database.
    ///
    /// # Errors
    ///
    /// This function will return an error if it fails to connect to the database
    /// or listen for notifications.
    #[cfg(feature = "sqlx-listener")]
    pub async fn listen(&self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        let mut listener = sqlx::postgres::PgListener::connect_with(pool).await?;
        listener.listen(&self.channel).await?;
        debug!("Started listening on channel '{}'", self.channel);

        loop {
            match listener.recv().await {
                Ok(notification) => {
                    self.process_notification(notification.payload()).await;
                }
                Err(e) => {
                    error!("Error receiving notification: {}", e);
                    // Optional: add a delay before trying to reconnect
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    // Attempt to reconnect
                    match sqlx::postgres::PgListener::connect_with(pool).await {
                        Ok(new_listener) => {
                            listener = new_listener;
                            if let Err(listen_err) = listener.listen(&self.channel).await {
                                error!(
                                    "Failed to re-listen on channel '{}': {}",
                                    self.channel, listen_err
                                );
                                return Err(listen_err);
                            }
                            debug!("Reconnected and listening on channel '{}'", self.channel);
                        }
                        Err(connect_err) => {
                            error!("Failed to reconnect to database: {}", connect_err);
                            // Continue loop to retry connection
                        }
                    }
                }
            }
        }
    }
}

impl Default for CacheNotificationListener {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_serialization() {
        let notif = CacheNotification {
            table: "users".to_string(),
            action: "insert".to_string(),
            id: Uuid::new_v4(),
            data: Some(serde_json::json!({
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "name": "Alice"
            })),
        };

        let json = serde_json::to_string(&notif).unwrap();
        let deserialized: CacheNotification = serde_json::from_str(&json).unwrap();

        assert_eq!(notif.table, deserialized.table);
        assert_eq!(notif.action, deserialized.action);
        assert_eq!(notif.id, deserialized.id);
    }
}