use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// A trait for models that have a primary key of type Uuid.
pub trait HasPrimaryKey {
    /// Returns the primary key of the model.
    fn primary_key(&self) -> Uuid;
}

/// A trait for models that have secondary indexes.
pub trait Indexable {
    /// Returns a map of i64 secondary keys.
    /// The key of the map is the name of the index.
    fn i64_keys(&self) -> HashMap<String, Option<i64>>;

    /// Returns a map of Uuid secondary keys.
    /// The key of the map is the name of the index.
    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>>;
}

/// A trait for models that have a validity start time.
/// When implemented, the cache can check if an entity is not yet valid.
pub trait ValidFrom {
    /// Returns the timestamp from which this entity becomes valid.
    /// If None, the entity is considered valid from the beginning of time.
    fn valid_from(&self) -> Option<DateTime<Utc>>;
}

/// A trait for models that have a validity end time.
/// When implemented, the cache can evict entities that are no longer valid.
pub trait ValidTo {
    /// Returns the timestamp until which this entity remains valid.
    /// If None, the entity is considered valid indefinitely.
    fn valid_to(&self) -> Option<DateTime<Utc>>;
}