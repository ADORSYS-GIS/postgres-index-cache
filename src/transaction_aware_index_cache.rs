use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;
use uuid::Uuid;

use crate::index_cache::IdxModelCache;
use crate::traits::{HasPrimaryKey, Indexable};
use postgres_unit_of_work::{TransactionAware, TransactionResult};

/// A trait alias for types that can be used in the cache
pub trait IdxModel: Clone + HasPrimaryKey + Indexable + Send + Sync + Debug {}
impl<T> IdxModel for T where T: Clone + HasPrimaryKey + Indexable + Send + Sync + Debug {}

/// A transaction-aware wrapper around IdxModelCache that stages changes
/// and applies them only on commit.
pub struct TransactionAwareIdxModelCache<T>
where
    T: IdxModel,
{
    shared_cache: Arc<RwLock<IdxModelCache<T>>>,
    local_additions: RwLock<HashMap<Uuid, T>>,
    local_updates: RwLock<HashMap<Uuid, T>>,
    local_deletions: RwLock<HashSet<Uuid>>,
}

impl<T> TransactionAwareIdxModelCache<T>
where
    T: IdxModel,
{
    /// Creates a new transaction-aware cache wrapper
    pub fn new(shared_cache: Arc<RwLock<IdxModelCache<T>>>) -> Self {
        Self {
            shared_cache,
            local_additions: RwLock::new(HashMap::new()),
            local_updates: RwLock::new(HashMap::new()),
            local_deletions: RwLock::new(HashSet::new()),
        }
    }

    /// Stages an item for addition to the cache
    pub fn add(&self, item: T) {
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
    pub fn get_by_primary(&self, primary_key: &Uuid) -> Option<T> {
        if self.local_deletions.read().contains(primary_key) {
            return None;
        }
        if let Some(item) = self.local_additions.read().get(primary_key) {
            return Some(item.clone());
        }
        if let Some(item) = self.local_updates.read().get(primary_key) {
            return Some(item.clone());
        }
        self.shared_cache.read().get_by_primary(primary_key)
    }

    /// Gets items by i64 index, considering staged changes
    pub fn get_by_i64_index(&self, key: &str, value: &i64) -> Vec<T> {
        let mut result_map = HashMap::new();

        // 1. Get from shared cache
        if let Some(pks) = self.shared_cache.read().get_by_i64_index(key, value) {
            for pk in pks {
                // Use get_by_primary which is transaction-aware for updates and deletions of these specific items
                if let Some(item) = self.get_by_primary(pk) {
                    result_map.insert(*pk, item);
                }
            }
        }

        // 2. Check local additions for new items that match
        for item in self.local_additions.read().values() {
            if let Some(Some(item_value)) = item.i64_keys().get(key) {
                if item_value == value {
                    result_map.insert(item.primary_key(), item.clone());
                }
            }
        }
        
        // 3. Check local updates for items that might now match or un-match
        for item in self.local_updates.read().values() {
            if let Some(Some(item_value)) = item.i64_keys().get(key) {
                if item_value == value {
                    // It matches now, so add/update it
                    result_map.insert(item.primary_key(), item.clone());
                } else {
                    // It doesn't match anymore, so remove it
                    result_map.remove(&item.primary_key());
                }
            } else {
                // The key was removed in the update, so it doesn't match
                result_map.remove(&item.primary_key());
            }
        }

        result_map.into_values().collect()
    }

    /// Gets items by uuid index, considering staged changes
    pub fn get_by_uuid_index(&self, key: &str, value: &Uuid) -> Vec<T> {
        let mut result_map = HashMap::new();

        // 1. Get from shared cache
        if let Some(pks) = self.shared_cache.read().get_by_uuid_index(key, value) {
            for pk in pks {
                // Use get_by_primary which is transaction-aware for updates and deletions of these specific items
                if let Some(item) = self.get_by_primary(pk) {
                    result_map.insert(*pk, item);
                }
            }
        }

        // 2. Check local additions for new items that match
        for item in self.local_additions.read().values() {
            if let Some(Some(item_value)) = item.uuid_keys().get(key) {
                if item_value == value {
                    result_map.insert(item.primary_key(), item.clone());
                }
            }
        }
        
        // 3. Check local updates for items that might now match or un-match
        for item in self.local_updates.read().values() {
            if let Some(Some(item_value)) = item.uuid_keys().get(key) {
                if item_value == value {
                    // It matches now, so add/update it
                    result_map.insert(item.primary_key(), item.clone());
                } else {
                    // It doesn't match anymore, so remove it
                    result_map.remove(&item.primary_key());
                }
            } else {
                // The key was removed in the update, so it doesn't match
                result_map.remove(&item.primary_key());
            }
        }

        result_map.into_values().collect()
    }

    /// Checks if the cache contains an item by primary key, considering staged changes
    pub fn contains_primary(&self, primary_key: &Uuid) -> bool {
        if self.local_deletions.read().contains(primary_key) {
            return false;
        }
        if self.local_additions.read().contains_key(primary_key) {
            return true;
        }
        if self.local_updates.read().contains_key(primary_key) {
            return true;
        }
        self.shared_cache.read().contains_primary(primary_key)
    }
}

#[async_trait]
impl<T> TransactionAware for TransactionAwareIdxModelCache<T>
where
    T: IdxModel,
{
    async fn on_commit(&self) -> TransactionResult<()> {
        let mut shared = self.shared_cache.write();
        for item in self.local_additions.read().values() {
            shared.add(item.clone());
        }
        for item in self.local_updates.read().values() {
            shared.update(item.clone());
        }
        for id in self.local_deletions.read().iter() {
            shared.remove(id);
        }
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