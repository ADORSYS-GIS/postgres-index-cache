use std::collections::HashMap;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use postgres_index_cache::{HasPrimaryKey, Indexable};

// Hash function to compute i64 hash values
pub fn hash_as_i64<T: Serialize>(data: &T) -> i64 {
    use std::hash::Hasher;
    use twox_hash::XxHash64;
    
    let mut hasher = XxHash64::with_seed(0);
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(data, &mut cbor).unwrap();
    hasher.write(&cbor);
    hasher.finish() as i64
}

/// Sample User entity for testing
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
}

impl User {
    pub fn new(username: String, email: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            username,
            email,
        }
    }
}

/// UserIndexCache - the cache model for User with hash fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserIndexCache {
    pub id: Uuid,
    pub username_hash: i64,
    pub email_hash: i64,
}

impl UserIndexCache {
    pub fn new(id: Uuid, username: &str, email: &str) -> Self {
        Self {
            id,
            username_hash: hash_as_i64(&username),
            email_hash: hash_as_i64(&email),
        }
    }
    
    pub fn from_user(user: &User) -> Self {
        Self::new(user.id, &user.username, &user.email)
    }
}

impl HasPrimaryKey for UserIndexCache {
    fn primary_key(&self) -> Uuid {
        self.id
    }
}

impl Indexable for UserIndexCache {
    fn i64_keys(&self) -> HashMap<String, Option<i64>> {
        let mut map = HashMap::new();
        map.insert("username_hash".to_string(), Some(self.username_hash));
        map.insert("email_hash".to_string(), Some(self.email_hash));
        map
    }

    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>> {
        HashMap::new()
    }
}

/// Sample Product entity for testing
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Product {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_name: String,
}

impl Product {
    pub fn new(user_id: Uuid, product_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            product_name,
        }
    }
}

/// ProductIndexCache - the cache model for Product with hash fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductIndexCache {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_name_hash: i64,
}

impl ProductIndexCache {
    pub fn new(id: Uuid, user_id: Uuid, product_name: &str) -> Self {
        Self {
            id,
            user_id,
            product_name_hash: hash_as_i64(&product_name),
        }
    }
    
    pub fn from_product(product: &Product) -> Self {
        Self::new(product.id, product.user_id, &product.product_name)
    }
}

impl HasPrimaryKey for ProductIndexCache {
    fn primary_key(&self) -> Uuid {
        self.id
    }
}

impl Indexable for ProductIndexCache {
    fn i64_keys(&self) -> HashMap<String, Option<i64>> {
        let mut map = HashMap::new();
        map.insert("product_name_hash".to_string(), Some(self.product_name_hash));
        map
    }

    fn uuid_keys(&self) -> HashMap<String, Option<Uuid>> {
        let mut map = HashMap::new();
        map.insert("user_id".to_string(), Some(self.user_id));
        map
    }
}