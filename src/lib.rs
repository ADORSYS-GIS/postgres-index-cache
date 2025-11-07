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
mod cache;
mod transaction_aware_cache;
mod listener;

pub use error::{CacheError, CacheResult};
pub use traits::{HasPrimaryKey, Indexable};
pub use cache::IdxModelCache;
pub use transaction_aware_cache::TransactionAwareIdxModelCache;

// Re-export listener components
pub use listener::{
    CacheNotification,
    CacheNotificationHandler,
    CacheNotificationListener,
    IndexCacheHandler,
    DEFAULT_CACHE_CHANNEL,
};

// Re-export TransactionAware from postgres-unit-of-work for convenience
pub use postgres_unit_of_work::TransactionAware;