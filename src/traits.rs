use std::collections::HashMap;
use uuid::Uuid;

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