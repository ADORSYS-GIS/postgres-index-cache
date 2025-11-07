use std::collections::HashMap;
use std::fmt::Debug;
use uuid::Uuid;

use crate::error::CacheError;
use crate::traits::{HasPrimaryKey, Indexable};

/// A generic cache for index models.
#[derive(Debug, Clone)]
pub struct IdxModelCache<T: HasPrimaryKey + Indexable + Clone> {
    by_id: HashMap<Uuid, T>,
    i64_indexes: HashMap<String, HashMap<i64, Vec<Uuid>>>,
    uuid_indexes: HashMap<String, HashMap<Uuid, Vec<Uuid>>>,
}

impl<T: HasPrimaryKey + Indexable + Clone + Debug> IdxModelCache<T> {
    /// Creates a new cache from a vector of items.
    pub fn new(items: Vec<T>) -> Result<Self, CacheError> {
        let mut by_id = HashMap::new();
        let mut i64_indexes: HashMap<String, HashMap<i64, Vec<Uuid>>> = HashMap::new();
        let mut uuid_indexes: HashMap<String, HashMap<Uuid, Vec<Uuid>>> = HashMap::new();

        for item in items {
            let primary_key = item.primary_key();
            if by_id.contains_key(&primary_key) {
                return Err(CacheError::DuplicatePrimaryKey(primary_key.to_string()));
            }

            // i64 indexes
            for (key_name, key_value) in item.i64_keys() {
                if let Some(value) = key_value {
                    i64_indexes
                        .entry(key_name)
                        .or_default()
                        .entry(value)
                        .or_default()
                        .push(primary_key);
                }
            }

            // uuid indexes
            for (key_name, key_value) in item.uuid_keys() {
                if let Some(value) = key_value {
                    uuid_indexes
                        .entry(key_name)
                        .or_default()
                        .entry(value)
                        .or_default()
                        .push(primary_key);
                }
            }

            by_id.insert(primary_key, item);
        }

        Ok(IdxModelCache {
            by_id,
            i64_indexes,
            uuid_indexes,
        })
    }

    /// Adds an item to the cache. If the item already exists, it will be updated.
    pub fn add(&mut self, item: T) {
        let primary_key = item.primary_key();
        if self.by_id.contains_key(&primary_key) {
            self.update(item);
            return;
        }

        // i64 indexes
        for (key_name, key_value) in item.i64_keys() {
            if let Some(value) = key_value {
                self.i64_indexes
                    .entry(key_name)
                    .or_default()
                    .entry(value)
                    .or_default()
                    .push(primary_key);
            }
        }

        // uuid indexes
        for (key_name, key_value) in item.uuid_keys() {
            if let Some(value) = key_value {
                self.uuid_indexes
                    .entry(key_name)
                    .or_default()
                    .entry(value)
                    .or_default()
                    .push(primary_key);
            }
        }

        self.by_id.insert(primary_key, item);
    }

    /// Removes an item from the cache by its primary key.
    pub fn remove(&mut self, primary_key: &Uuid) -> Option<T> {
        if let Some(item) = self.by_id.remove(primary_key) {
            // i64 indexes
            for (key_name, key_value) in item.i64_keys() {
                if let Some(value) = key_value {
                    if let Some(index) = self.i64_indexes.get_mut(&key_name) {
                        if let Some(ids) = index.get_mut(&value) {
                            ids.retain(|&id| id != *primary_key);
                            if ids.is_empty() {
                                index.remove(&value);
                            }
                        }
                        if index.is_empty() {
                            self.i64_indexes.remove(&key_name);
                        }
                    }
                }
            }

            // uuid indexes
            for (key_name, key_value) in item.uuid_keys() {
                if let Some(value) = key_value {
                    if let Some(index) = self.uuid_indexes.get_mut(&key_name) {
                        if let Some(ids) = index.get_mut(&value) {
                            ids.retain(|&id| id != *primary_key);
                            if ids.is_empty() {
                                index.remove(&value);
                            }
                        }
                        if index.is_empty() {
                            self.uuid_indexes.remove(&key_name);
                        }
                    }
                }
            }
            return Some(item);
        }
        None
    }

    /// Updates an item in the cache.
    pub fn update(&mut self, item: T) {
        self.remove(&item.primary_key());
        self.add(item);
    }

    /// Checks if the cache contains an item with the given primary key.
    pub fn contains_primary(&self, primary_key: &Uuid) -> bool {
        self.by_id.contains_key(primary_key)
    }

    /// Gets an item from the cache by its primary key.
    pub fn get_by_primary(&self, primary_key: &Uuid) -> Option<T> {
        self.by_id.get(primary_key).cloned()
    }

    /// Gets a vector of primary keys by a secondary i64 index.
    pub fn get_by_i64_index(&self, index_name: &str, key: &i64) -> Option<&Vec<Uuid>> {
        self.i64_indexes.get(index_name).and_then(|index| index.get(key))
    }

    /// Gets a vector of primary keys by a secondary Uuid index.
    pub fn get_by_uuid_index(&self, index_name: &str, key: &Uuid) -> Option<&Vec<Uuid>> {
        self.uuid_indexes.get(index_name).and_then(|index| index.get(key))
    }

    /// Returns an iterator over the items in the cache.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.by_id.values()
    }
}