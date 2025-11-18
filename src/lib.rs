//! # Postgres Index Cache
//!
//! This crate provides in-memory caching functionality for PostgreSQL repository indexes.
//! It supports transaction-aware caching with automatic cache updates on commit/rollback.
//!
//! ## Key Components
//!
//! - `IdxModelCache`: Core cache structure for storing indexed models
//! - `TransactionAwareIdxModelCache`: Transaction-aware wrapper that stages changes
//! - `TransactionAware`: Trait for transaction lifecycle notifications (from postgres-unit-of-work)
//! - `HasPrimaryKey` and `Indexable`: Traits for cacheable models

mod error;
mod traits;
mod index_cache;
mod transaction_aware_index_cache;
mod listener;
mod db_init;
mod main_model_cache;
mod transaction_aware_main_model_cache;

pub use error::{CacheError, CacheResult};
pub use traits::{HasPrimaryKey, Indexable, ValidFrom, ValidTo};
pub use index_cache::IdxModelCache;
pub use transaction_aware_index_cache::TransactionAwareIdxModelCache;
pub use transaction_aware_main_model_cache::TransactionAwareMainModelCache;

// Re-export main model cache components
pub use main_model_cache::{
    MainModelCache,
    MainModelCacheHandler,
    CacheConfig,
    CacheStatistics,
    EvictionPolicy,
};

// Re-export listener components
pub use listener::{
    CacheNotification,
    CacheNotificationHandler,
    CacheNotificationListener,
    IndexCacheHandler,
    DEFAULT_CACHE_CHANNEL,
};

// Re-export database initialization functions
pub use db_init::{init_cache_triggers, cleanup_cache_triggers};

// Re-export TransactionAware from postgres-unit-of-work for convenience
pub use postgres_unit_of_work::TransactionAware;