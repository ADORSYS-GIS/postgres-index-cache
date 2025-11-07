# postgres-index-cache

A high-performance, transaction-aware in-memory caching library for PostgreSQL repository indexes in Rust.

## Overview

`postgres-index-cache` provides a generic caching solution for PostgreSQL repositories that need to maintain in-memory indexes for fast lookups. It supports transaction-aware caching with automatic cache updates on commit/rollback, making it ideal for use with Unit of Work patterns.

## Features

- **Generic Index Cache**: Store and retrieve models by primary key (UUID) and secondary indexes (i64 and UUID types)
- **Transaction-Aware**: Automatically stage changes during transactions and apply them only on commit
- **Thread-Safe**: Built with `parking_lot::RwLock` for concurrent access
- **Type-Safe**: Leverages Rust's type system with trait-based design
- **Async Support**: Async transaction lifecycle hooks via `async-trait`
- **Zero-Copy Reads**: Efficient read operations with minimal cloning

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
postgres-index-cache = "0.1.0"
```

## Core Components

### Traits

#### `HasPrimaryKey`
Models must implement this trait to be cacheable:
```rust
pub trait HasPrimaryKey {
    fn primary_key(&self) -> Uuid;
}
```

#### `Indexable`
Models must implement this trait to support secondary indexes:
```rust
pub trait Indexable {
    fn i64_keys(&self) -> HashMap<String, Option<i64>>;
    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>>;
}
```

#### `TransactionAware`
Components can implement this trait to receive transaction lifecycle notifications:
```rust
#[async_trait]
pub trait TransactionAware: Send + Sync {
    async fn on_commit(&self) -> CacheResult<()>;
    async fn on_rollback(&self) -> CacheResult<()>;
}
```

### Cache Types

#### `IdxModelCache<T>`
The core cache structure that stores models with primary key and secondary index lookups.

**Key Methods:**
- `new(items: Vec<T>)` - Create a cache from a vector of items
- `add(item: T)` - Add or update an item
- `remove(primary_key: &Uuid)` - Remove an item
- `update(item: T)` - Update an existing item
- `get_by_primary(primary_key: &Uuid)` - Get by primary key
- `get_by_i64_index(index_name: &str, key: &i64)` - Get by i64 index
- `get_by_uuid_index(index_name: &str, key: &Uuid)` - Get by UUID index
- `contains_primary(primary_key: &Uuid)` - Check existence

#### `TransactionAwareIdxModelCache<T>`
A transaction-aware wrapper that stages changes and applies them only on commit.

**Key Methods:**
- `new(shared_cache: Arc<RwLock<IdxModelCache<T>>>)` - Wrap an existing cache
- `add(item: T)` - Stage an addition
- `update(item: T)` - Stage an update
- `remove(primary_key: &Uuid)` - Stage a deletion
- `get_by_primary(primary_key: &Uuid)` - Get with staged changes
- `get_by_i64_index(key: &str, value: &i64)` - Get by i64 index with staged changes
- `get_by_uuid_index(key: &str, value: &Uuid)` - Get by UUID index with staged changes
- `contains_primary(primary_key: &Uuid)` - Check existence with staged changes

## Usage

### Basic Cache Usage

```rust
use postgres_index_cache::{IdxModelCache, HasPrimaryKey, Indexable};
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct Country {
    id: Uuid,
    iso2_hash: i64,
    name: String,
}

impl HasPrimaryKey for Country {
    fn primary_key(&self) -> Uuid {
        self.id
    }
}

impl Indexable for Country {
    fn i64_keys(&self) -> HashMap<String, Option<i64>> {
        let mut map = HashMap::new();
        map.insert("iso2_hash".to_string(), Some(self.iso2_hash));
        map
    }

    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>> {
        HashMap::new()
    }
}

// Create cache
let countries = vec![
    Country { id: Uuid::new_v4(), iso2_hash: 123, name: "USA".to_string() },
    Country { id: Uuid::new_v4(), iso2_hash: 456, name: "Canada".to_string() },
];

let cache = IdxModelCache::new(countries)?;

// Lookup by primary key
let country = cache.get_by_primary(&some_id);

// Lookup by secondary index
let countries_by_hash = cache.get_by_i64_index("iso2_hash", &123);
```

### Transaction-Aware Cache

```rust
use postgres_index_cache::{TransactionAwareIdxModelCache, TransactionAware};
use parking_lot::RwLock;
use std::sync::Arc;

// Initialize shared cache
let shared_cache = Arc::new(RwLock::new(IdxModelCache::new(countries)?));

// Create transaction-aware wrapper
let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());

// Stage changes during transaction
tx_cache.add(new_country);
tx_cache.update(updated_country);
tx_cache.remove(&deleted_id);

// Changes are visible in this transaction
let country = tx_cache.get_by_primary(&new_country.id); // Returns Some(new_country)

// On commit: apply changes to shared cache
tx_cache.on_commit().await?;

// On rollback: discard all staged changes
tx_cache.on_rollback().await?;
```

### Integration with Unit of Work

```rust
use postgres_index_cache::TransactionAware;

// Register with Unit of Work
unit_of_work.register_transaction_aware(
    Arc::new(tx_cache) as Arc<dyn TransactionAware>
);

// Changes will be automatically committed/rolled back
// when the unit of work commits/rolls back
```

## Error Handling

The library uses a custom error type:

```rust
pub enum CacheError {
    DuplicatePrimaryKey(String),
    CommitFailed(String),
    RollbackFailed(String),
    OperationFailed(String),
}

pub type CacheResult<T> = Result<T, CacheError>;
```

## Performance Considerations

- **Read Operations**: O(1) for primary key lookups, O(1) for index lookups
- **Write Operations**: O(k) where k is the number of indexes per model
- **Memory**: Stores one copy per model plus index overhead
- **Concurrency**: Uses `RwLock` for multiple concurrent readers

## Thread Safety

All cache types are thread-safe and can be shared across threads using `Arc`:

```rust
let shared_cache = Arc::new(RwLock::new(cache));
// Clone and use in multiple threads
```

## Dependencies

- `uuid` - UUID support with v4 generation and serialization
- `async-trait` - Async trait support
- `parking_lot` - High-performance RwLock
- `thiserror` - Error handling

## Development Dependencies

- `tokio` - Async runtime for testing (features: "full")

## License

See the workspace license.

## Related Projects

- [`postgres-unit-of-work`](../postgres-unit-of-work) - Unit of Work implementation for PostgreSQL
- [`ledger-banking-rust`](../ledger-banking-rust) - Banking ledger using this cache library

## Contributing

Contributions are welcome! Please ensure all tests pass and add tests for new features.

## Version

Current version: **0.1.0**

Rust Edition: **2021**