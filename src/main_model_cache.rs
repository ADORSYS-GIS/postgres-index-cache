use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

use crate::traits::{HasPrimaryKey, ValidFrom, ValidTo};
use crate::listener::{CacheNotification, CacheNotificationHandler};

/// Eviction policy for the cache
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least Recently Used - evicts the least recently accessed entry
    LRU,
    /// First In First Out - evicts the oldest entry
    FIFO,
}

/// Statistics for cache operations
#[derive(Debug)]
pub struct CacheStatistics {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
    invalidations: AtomicU64,
}

impl CacheStatistics {
    fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
        }
    }

    /// Get the number of cache hits
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get the number of cache misses
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get the number of evictions
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// Get the number of invalidations
    pub fn invalidations(&self) -> u64 {
        self.invalidations.load(Ordering::Relaxed)
    }

    /// Calculate the cache hit rate (hits / (hits + misses))
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits();
        let total = hits + self.misses();
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    fn record_invalidation(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }
}

/// Entry metadata for cache management
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    inserted_at: DateTime<Utc>,
    last_accessed: DateTime<Utc>,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        let now = Utc::now();
        Self {
            value,
            inserted_at: now,
            last_accessed: now,
        }
    }

    fn access(&mut self) {
        self.last_accessed = Utc::now();
    }
}

/// Configuration for MainModelCache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in the cache
    pub cache_size: usize,
    /// Eviction policy to use when cache is full
    pub eviction_policy: EvictionPolicy,
    /// Optional TTL for cache entries
    pub ttl: Option<Duration>,
}

impl CacheConfig {
    /// Create a new cache configuration
    pub fn new(cache_size: usize, eviction_policy: EvictionPolicy) -> Self {
        Self {
            cache_size,
            eviction_policy,
            ttl: None,
        }
    }

    /// Set the TTL for cache entries
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// A generic cache for main models with eviction policies
pub struct MainModelCache<T: HasPrimaryKey + Clone> {
    /// Main storage indexed by primary key
    entries: HashMap<Uuid, CacheEntry<T>>,
    /// Access order tracking (for LRU and FIFO)
    access_order: VecDeque<Uuid>,
    /// Configuration
    config: CacheConfig,
    /// Statistics
    statistics: CacheStatistics,
}

impl<T: HasPrimaryKey + Clone + Debug> MainModelCache<T> {
    /// Creates a new empty cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: VecDeque::new(),
            config,
            statistics: CacheStatistics::new(),
        }
    }

    /// Gets an item from the cache by its primary key
    /// Returns None if the item is not in cache or is no longer valid
    pub fn get(&mut self, primary_key: &Uuid) -> Option<T> {
        // Check if entry exists
        if let Some(entry) = self.entries.get(primary_key) {
            // Check TTL expiration
            let should_evict = if let Some(ttl) = self.config.ttl {
                let elapsed = Utc::now().signed_duration_since(entry.inserted_at);
                elapsed.to_std().ok().is_some_and(|d| d > ttl)
            } else {
                false
            };

            if should_evict {
                // Entry has expired, remove it
                let _ = entry; // Release borrow
                self.remove_internal(primary_key);
                self.statistics.record_miss();
                return None;
            }

            let result = entry.value.clone();
            let _ = entry; // Release borrow

            // Update access time and order
            if let Some(entry) = self.entries.get_mut(primary_key) {
                entry.access();
            }

            // Update access order for LRU policy
            if self.config.eviction_policy == EvictionPolicy::LRU {
                self.access_order.retain(|&id| id != *primary_key);
                self.access_order.push_back(*primary_key);
            }

            self.statistics.record_hit();
            Some(result)
        } else {
            self.statistics.record_miss();
            None
        }
    }

    /// Inserts or updates an item in the cache
    /// If the cache is full, evicts entries according to the eviction policy
    pub fn insert(&mut self, item: T) {
        let primary_key = item.primary_key();

        // If item already exists, update it
        if self.entries.contains_key(&primary_key) {
            self.update(item);
            return;
        }

        // Check if we need to evict
        while self.entries.len() >= self.config.cache_size && !self.access_order.is_empty() {
            self.evict_one();
        }

        // Insert the new entry
        let entry = CacheEntry::new(item);
        self.entries.insert(primary_key, entry);
        self.access_order.push_back(primary_key);
    }

    /// Updates an existing item in the cache
    /// If the item doesn't exist, it will be inserted
    pub fn update(&mut self, item: T) {
        let primary_key = item.primary_key();
        
        if let Some(entry) = self.entries.get_mut(&primary_key) {
            entry.value = item;
            entry.access();
            
            // Update access order for LRU
            if self.config.eviction_policy == EvictionPolicy::LRU {
                self.access_order.retain(|&id| id != primary_key);
                self.access_order.push_back(primary_key);
            }
        } else {
            self.insert(item);
        }
    }

    /// Removes an item from the cache by its primary key
    /// Returns the removed item if it existed
    pub fn remove(&mut self, primary_key: &Uuid) -> Option<T> {
        self.statistics.record_invalidation();
        self.remove_internal(primary_key)
    }

    /// Checks if the cache contains an item with the given primary key
    pub fn contains(&self, primary_key: &Uuid) -> bool {
        self.entries.contains_key(primary_key)
    }

    /// Returns the number of items currently in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all entries from the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    /// Gets the cache statistics
    pub fn statistics(&self) -> &CacheStatistics {
        &self.statistics
    }

    /// Gets the cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Evicts all expired or invalid entries from the cache
    /// This performs a lazy cleanup based on TTL only
    /// For validity checks with ValidFrom/ValidTo, use the extension methods
    pub fn evict_invalid(&mut self) -> usize {
        let mut to_remove = Vec::new();

        for (key, entry) in &self.entries {
            // Check TTL expiration
            if let Some(ttl) = self.config.ttl {
                let elapsed = Utc::now().signed_duration_since(entry.inserted_at);
                if elapsed.to_std().ok().is_some_and(|d| d > ttl) {
                    to_remove.push(*key);
                }
            }
        }

        let count = to_remove.len();
        for key in to_remove {
            self.remove_internal(&key);
            self.statistics.record_eviction();
        }

        count
    }

    /// Internal remove that doesn't record statistics
    fn remove_internal(&mut self, primary_key: &Uuid) -> Option<T> {
        self.access_order.retain(|&id| id != *primary_key);
        self.entries.remove(primary_key).map(|entry| entry.value)
    }

    /// Evicts one entry based on the eviction policy
    fn evict_one(&mut self) {
        let key_to_evict = match self.config.eviction_policy {
            EvictionPolicy::LRU => {
                // Remove the least recently used (front of the deque)
                self.access_order.pop_front()
            }
            EvictionPolicy::FIFO => {
                // Remove the oldest inserted (front of the deque)
                self.access_order.pop_front()
            }
        };

        if let Some(key) = key_to_evict {
            self.entries.remove(&key);
            self.statistics.record_eviction();
        }
    }

}

/// Extension trait for MainModelCache when T implements ValidFrom
impl<T: HasPrimaryKey + Clone + Debug + ValidFrom> MainModelCache<T> {
    /// Checks if an item is valid based on ValidFrom
    pub fn is_valid_from(&self, item: &T) -> bool {
        if let Some(valid_from) = item.valid_from() {
            Utc::now() >= valid_from
        } else {
            true
        }
    }
}

/// Extension trait for MainModelCache when T implements ValidTo
impl<T: HasPrimaryKey + Clone + Debug + ValidTo> MainModelCache<T> {
    /// Checks if an item is valid based on ValidTo
    pub fn is_valid_to(&self, item: &T) -> bool {
        if let Some(valid_to) = item.valid_to() {
            Utc::now() <= valid_to
        } else {
            true
        }
    }
}

/// Extension trait for MainModelCache when T implements both ValidFrom and ValidTo
impl<T: HasPrimaryKey + Clone + Debug + ValidFrom + ValidTo> MainModelCache<T> {
    /// Checks if an item is currently valid based on both ValidFrom and ValidTo
    pub fn is_fully_valid(&self, item: &T) -> bool {
        self.is_valid_from(item) && self.is_valid_to(item)
    }

    /// Gets an item from the cache with full validity checking
    pub fn get_with_validity_check(&mut self, primary_key: &Uuid) -> Option<T> {
        // First check validity without mutable borrow
        if let Some(entry) = self.entries.get(primary_key) {
            // Check full validity
            if !self.is_fully_valid(&entry.value) {
                let _ = entry; // Release borrow
                self.remove_internal(primary_key);
                self.statistics.record_miss();
                return None;
            }

            // Check TTL expiration
            let should_evict = if let Some(ttl) = self.config.ttl {
                let elapsed = Utc::now().signed_duration_since(entry.inserted_at);
                elapsed.to_std().ok().is_some_and(|d| d > ttl)
            } else {
                false
            };

            if should_evict {
                let _ = entry; // Release borrow
                self.remove_internal(primary_key);
                self.statistics.record_miss();
                return None;
            }

            let result = entry.value.clone();
            let _ = entry; // Release borrow

            // Now update with mutable borrow
            if let Some(entry) = self.entries.get_mut(primary_key) {
                entry.access();
            }

            if self.config.eviction_policy == EvictionPolicy::LRU {
                self.access_order.retain(|&id| id != *primary_key);
                self.access_order.push_back(*primary_key);
            }

            self.statistics.record_hit();
            Some(result)
        } else {
            self.statistics.record_miss();
            None
        }
    }

    /// Evicts all expired or invalid entries from the cache
    /// This performs a lazy cleanup based on ValidFrom, ValidTo, and TTL
    pub fn evict_invalid_with_validity(&mut self) -> usize {
        let mut to_remove = Vec::new();

        for (key, entry) in &self.entries {
            let mut should_remove = false;

            // Check validity
            if !self.is_fully_valid(&entry.value) {
                should_remove = true;
            }

            // Check TTL expiration
            if let Some(ttl) = self.config.ttl {
                let elapsed = Utc::now().signed_duration_since(entry.inserted_at);
                if elapsed.to_std().ok().is_some_and(|d| d > ttl) {
                    should_remove = true;
                }
            }

            if should_remove {
                to_remove.push(*key);
            }
        }

        let count = to_remove.len();
        for key in to_remove {
            self.remove_internal(&key);
            self.statistics.record_eviction();
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestEntity {
        id: Uuid,
        value: String,
    }

    impl HasPrimaryKey for TestEntity {
        fn primary_key(&self) -> Uuid {
            self.id
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let mut cache = MainModelCache::new(config);

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };

        cache.insert(entity.clone());
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(&entity.id).unwrap();
        assert_eq!(retrieved.value, "test");
    }

    #[test]
    fn test_lru_eviction() {
        let config = CacheConfig::new(2, EvictionPolicy::LRU);
        let mut cache = MainModelCache::new(config);

        let entity1 = TestEntity {
            id: Uuid::new_v4(),
            value: "first".to_string(),
        };
        let entity2 = TestEntity {
            id: Uuid::new_v4(),
            value: "second".to_string(),
        };
        let entity3 = TestEntity {
            id: Uuid::new_v4(),
            value: "third".to_string(),
        };

        cache.insert(entity1.clone());
        cache.insert(entity2.clone());

        // Access entity1 to make it more recently used
        cache.get(&entity1.id);

        // Insert entity3, should evict entity2 (least recently used)
        cache.insert(entity3.clone());

        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&entity1.id));
        assert!(!cache.contains(&entity2.id));
        assert!(cache.contains(&entity3.id));
    }

    #[test]
    fn test_fifo_eviction() {
        let config = CacheConfig::new(2, EvictionPolicy::FIFO);
        let mut cache = MainModelCache::new(config);

        let entity1 = TestEntity {
            id: Uuid::new_v4(),
            value: "first".to_string(),
        };
        let entity2 = TestEntity {
            id: Uuid::new_v4(),
            value: "second".to_string(),
        };
        let entity3 = TestEntity {
            id: Uuid::new_v4(),
            value: "third".to_string(),
        };

        cache.insert(entity1.clone());
        cache.insert(entity2.clone());

        // Access entity1 (shouldn't matter for FIFO)
        cache.get(&entity1.id);

        // Insert entity3, should evict entity1 (first in)
        cache.insert(entity3.clone());

        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&entity1.id));
        assert!(cache.contains(&entity2.id));
        assert!(cache.contains(&entity3.id));
    }

    #[test]
    fn test_statistics() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let mut cache = MainModelCache::new(config);

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };

        cache.insert(entity.clone());
        
        // Should record a hit
        cache.get(&entity.id);
        assert_eq!(cache.statistics().hits(), 1);

        // Should record a miss
        cache.get(&Uuid::new_v4());
        assert_eq!(cache.statistics().misses(), 1);

        assert_eq!(cache.statistics().hit_rate(), 0.5);
    }
}

/// A notification handler for MainModelCache
pub struct MainModelCacheHandler<T: HasPrimaryKey + Clone + Send + Sync + 'static> {
    table_name: String,
    cache: Arc<RwLock<MainModelCache<T>>>,
}

impl<T: HasPrimaryKey + Clone + Send + Sync + 'static> MainModelCacheHandler<T> {
    /// Create a new handler for the given cache
    pub fn new(table_name: String, cache: Arc<RwLock<MainModelCache<T>>>) -> Self {
        Self { table_name, cache }
    }
}

#[async_trait]
impl<T: HasPrimaryKey + Clone + Send + Sync + Debug + 'static> CacheNotificationHandler
    for MainModelCacheHandler<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    async fn handle_notification(&self, notification: CacheNotification) {
        tracing::debug!(
            "MainModelCache: Handling notification for table '{}': action={}, id={}",
            notification.table, notification.action, notification.id
        );

        match notification.action.as_str() {
            "insert" | "update" => {
                if let Some(data) = notification.data {
                    match serde_json::from_value::<T>(data) {
                        Ok(item) => {
                            let mut cache = self.cache.write();
                            if notification.action == "insert" {
                                cache.insert(item);
                                tracing::debug!("MainModelCache: Added item {} to cache", notification.id);
                            } else {
                                cache.update(item);
                                tracing::debug!("MainModelCache: Updated item {} in cache", notification.id);
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "MainModelCache: Failed to deserialize data for {}: {}",
                                notification.table, e
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        "MainModelCache: No data provided for {} operation on table {}",
                        notification.action, notification.table
                    );
                }
            }
            "delete" => {
                let mut cache = self.cache.write();
                cache.remove(&notification.id);
                tracing::debug!("MainModelCache: Removed item {} from cache", notification.id);
            }
            _ => {
                tracing::warn!(
                    "MainModelCache: Unknown action '{}' for table '{}'",
                    notification.action, notification.table
                );
            }
        }
    }

    fn table_name(&self) -> &str {
        &self.table_name
    }
}