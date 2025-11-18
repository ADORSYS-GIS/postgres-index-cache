use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;
use uuid::Uuid;

use crate::main_model_cache::MainModelCache;
use crate::traits::HasPrimaryKey;
use postgres_unit_of_work::{TransactionAware, TransactionResult};

/// A trait alias for types that can be used in the main model cache
pub trait MainModel: Clone + HasPrimaryKey + Send + Sync + Debug {}
impl<T> MainModel for T where T: Clone + HasPrimaryKey + Send + Sync + Debug {}

/// A transaction-aware wrapper around MainModelCache that stages changes
/// and applies them only on commit.
pub struct TransactionAwareMainModelCache<T>
where
    T: MainModel,
{
    shared_cache: Arc<RwLock<MainModelCache<T>>>,
    local_additions: RwLock<HashMap<Uuid, T>>,
    local_updates: RwLock<HashMap<Uuid, T>>,
    local_deletions: RwLock<HashSet<Uuid>>,
}

impl<T> TransactionAwareMainModelCache<T>
where
    T: MainModel,
{
    /// Creates a new transaction-aware cache wrapper
    pub fn new(shared_cache: Arc<RwLock<MainModelCache<T>>>) -> Self {
        Self {
            shared_cache,
            local_additions: RwLock::new(HashMap::new()),
            local_updates: RwLock::new(HashMap::new()),
            local_deletions: RwLock::new(HashSet::new()),
        }
    }

    /// Stages an item for addition to the cache
    pub fn insert(&self, item: T) {
        let primary_key = item.primary_key();
        self.local_deletions.write().remove(&primary_key);
        self.local_additions.write().insert(primary_key, item);
    }

    /// Stages an item for update in the cache
    pub fn update(&self, item: T) {
        let primary_key = item.primary_key();
        self.local_deletions.write().remove(&primary_key);
        if let Some(local_item) = self.local_additions.write().get_mut(&primary_key) {
            *local_item = item;
            return;
        }
        self.local_updates.write().insert(primary_key, item);
    }

    /// Stages an item for removal from the cache
    pub fn remove(&self, primary_key: &Uuid) {
        if self.local_additions.write().remove(primary_key).is_none() {
            self.local_deletions.write().insert(*primary_key);
        }
        self.local_updates.write().remove(primary_key);
    }

    /// Gets an item by primary key, considering staged changes
    /// Note: This returns None for items in the cache since MainModelCache::get requires &mut self
    /// For transactional reads, check local changes first, then fall back to checking contains
    pub fn get(&self, primary_key: &Uuid) -> Option<T> {
        // Check if marked for deletion
        if self.local_deletions.read().contains(primary_key) {
            return None;
        }
        
        // Check local additions first
        if let Some(item) = self.local_additions.read().get(primary_key) {
            return Some(item.clone());
        }
        
        // Check local updates
        if let Some(item) = self.local_updates.read().get(primary_key) {
            return Some(item.clone());
        }
        
        // For shared cache, we can't call get() as it requires &mut
        // Instead, we check if it exists and return None
        // The caller should use contains() to check existence
        None
    }

    /// Checks if the cache contains an item by primary key, considering staged changes
    pub fn contains(&self, primary_key: &Uuid) -> bool {
        if self.local_deletions.read().contains(primary_key) {
            return false;
        }
        if self.local_additions.read().contains_key(primary_key) {
            return true;
        }
        if self.local_updates.read().contains_key(primary_key) {
            return true;
        }
        self.shared_cache.read().contains(primary_key)
    }

    /// Clears all staged changes (useful for testing or manual rollback)
    pub fn clear_staged(&self) {
        self.local_additions.write().clear();
        self.local_updates.write().clear();
        self.local_deletions.write().clear();
    }

    /// Returns the number of staged additions
    pub fn staged_additions_count(&self) -> usize {
        self.local_additions.read().len()
    }

    /// Returns the number of staged updates
    pub fn staged_updates_count(&self) -> usize {
        self.local_updates.read().len()
    }

    /// Returns the number of staged deletions
    pub fn staged_deletions_count(&self) -> usize {
        self.local_deletions.read().len()
    }
}

#[async_trait]
impl<T> TransactionAware for TransactionAwareMainModelCache<T>
where
    T: MainModel,
{
    async fn on_commit(&self) -> TransactionResult<()> {
        let mut shared = self.shared_cache.write();
        
        // Apply additions
        for item in self.local_additions.read().values() {
            shared.insert(item.clone());
        }
        
        // Apply updates
        for item in self.local_updates.read().values() {
            shared.update(item.clone());
        }
        
        // Apply deletions
        for id in self.local_deletions.read().iter() {
            shared.remove(id);
        }
        
        // Clear staged changes
        self.local_additions.write().clear();
        self.local_updates.write().clear();
        self.local_deletions.write().clear();
        
        Ok(())
    }

    async fn on_rollback(&self) -> TransactionResult<()> {
        self.local_additions.write().clear();
        self.local_updates.write().clear();
        self.local_deletions.write().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::main_model_cache::{CacheConfig, EvictionPolicy};

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

    #[tokio::test]
    async fn test_transaction_aware_insert() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };

        // Insert in transaction
        tx_cache.insert(entity.clone());
        
        // Should be visible in local state
        assert!(tx_cache.contains(&entity.id));
        assert_eq!(tx_cache.staged_additions_count(), 1);
        
        // Should not be in shared cache yet
        assert!(!shared_cache.read().contains(&entity.id));

        // Commit
        tx_cache.on_commit().await.unwrap();
        
        // Now should be in shared cache
        assert!(shared_cache.read().contains(&entity.id));
        assert_eq!(tx_cache.staged_additions_count(), 0);
    }

    #[tokio::test]
    async fn test_transaction_aware_update() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        
        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "original".to_string(),
        };
        
        // Add to shared cache
        shared_cache.write().insert(entity.clone());

        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        // Update in transaction
        let updated_entity = TestEntity {
            id: entity.id,
            value: "updated".to_string(),
        };
        tx_cache.update(updated_entity.clone());
        
        assert_eq!(tx_cache.staged_updates_count(), 1);

        // Commit
        tx_cache.on_commit().await.unwrap();
        
        assert_eq!(tx_cache.staged_updates_count(), 0);
    }

    #[tokio::test]
    async fn test_transaction_aware_remove() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        
        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };
        
        // Add to shared cache
        shared_cache.write().insert(entity.clone());
        assert!(shared_cache.read().contains(&entity.id));

        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        // Remove in transaction
        tx_cache.remove(&entity.id);
        
        // Should be marked as deleted locally
        assert!(!tx_cache.contains(&entity.id));
        assert_eq!(tx_cache.staged_deletions_count(), 1);
        
        // Should still be in shared cache
        assert!(shared_cache.read().contains(&entity.id));

        // Commit
        tx_cache.on_commit().await.unwrap();
        
        // Now should be removed from shared cache
        assert!(!shared_cache.read().contains(&entity.id));
        assert_eq!(tx_cache.staged_deletions_count(), 0);
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };

        // Insert in transaction
        tx_cache.insert(entity.clone());
        assert_eq!(tx_cache.staged_additions_count(), 1);

        // Rollback
        tx_cache.on_rollback().await.unwrap();
        
        // Changes should be discarded
        assert_eq!(tx_cache.staged_additions_count(), 0);
        assert!(!shared_cache.read().contains(&entity.id));
    }

    #[tokio::test]
    async fn test_update_replaces_addition() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "original".to_string(),
        };

        // Insert then update in same transaction
        tx_cache.insert(entity.clone());
        
        let updated_entity = TestEntity {
            id: entity.id,
            value: "updated".to_string(),
        };
        tx_cache.update(updated_entity.clone());
        
        // Should only have one addition, not an update
        assert_eq!(tx_cache.staged_additions_count(), 1);
        assert_eq!(tx_cache.staged_updates_count(), 0);
        
        // The addition should have the updated value
        assert_eq!(tx_cache.get(&entity.id).unwrap().value, "updated");
    }

    #[tokio::test]
    async fn test_remove_cancels_addition() {
        let config = CacheConfig::new(10, EvictionPolicy::LRU);
        let shared_cache = Arc::new(RwLock::new(MainModelCache::new(config)));
        let tx_cache = TransactionAwareMainModelCache::new(shared_cache.clone());

        let entity = TestEntity {
            id: Uuid::new_v4(),
            value: "test".to_string(),
        };

        // Insert then remove in same transaction
        tx_cache.insert(entity.clone());
        tx_cache.remove(&entity.id);
        
        // Should have no staged changes
        assert_eq!(tx_cache.staged_additions_count(), 0);
        assert_eq!(tx_cache.staged_deletions_count(), 0);
        
        // Commit should be a no-op
        tx_cache.on_commit().await.unwrap();
        assert!(!shared_cache.read().contains(&entity.id));
    }
}