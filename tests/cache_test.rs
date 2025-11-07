mod common;

use common::{UserIndexCache, ProductIndexCache, User, Product};
use postgres_index_cache::{IdxModelCache, TransactionAwareIdxModelCache};
use parking_lot::RwLock;
use std::sync::Arc;

#[test]
fn test_basic_cache_operations() {
    // Create test users
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    
    let user_cache1 = UserIndexCache::from_user(&user1);
    let user_cache2 = UserIndexCache::from_user(&user2);
    
    // Create cache with initial items
    let mut cache = IdxModelCache::new(vec![user_cache1.clone()]).unwrap();
    
    // Test get by primary key
    let retrieved = cache.get_by_primary(&user1.id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, user1.id);
    
    // Test contains
    assert!(cache.contains_primary(&user1.id));
    assert!(!cache.contains_primary(&user2.id));
    
    // Test add
    cache.add(user_cache2.clone());
    assert!(cache.contains_primary(&user2.id));
    
    // Test remove
    let removed = cache.remove(&user1.id);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, user1.id);
    assert!(!cache.contains_primary(&user1.id));
    
    // Test update
    let mut updated_user_cache2 = user_cache2.clone();
    updated_user_cache2.email_hash = 999999;
    cache.update(updated_user_cache2.clone());
    let retrieved = cache.get_by_primary(&user2.id).unwrap();
    assert_eq!(retrieved.email_hash, 999999);
}

#[test]
fn test_i64_index_queries() {
    // Create test users with known hashes
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    let user3 = User::new("alice".to_string(), "alice2@example.com".to_string()); // Same username, different email
    
    let user_cache1 = UserIndexCache::from_user(&user1);
    let user_cache2 = UserIndexCache::from_user(&user2);
    let user_cache3 = UserIndexCache::from_user(&user3);
    
    // Create cache
    let cache = IdxModelCache::new(vec![
        user_cache1.clone(),
        user_cache2.clone(),
        user_cache3.clone(),
    ]).unwrap();
    
    // Test get by username_hash (alice and alice should have same hash)
    let alice_hash = user_cache1.username_hash;
    let results = cache.get_by_i64_index("username_hash", &alice_hash);
    assert!(results.is_some());
    let results = results.unwrap();
    assert_eq!(results.len(), 2); // user1 and user3 have same username
    
    // Test get by email_hash (should be unique)
    let email_hash = user_cache1.email_hash;
    let results = cache.get_by_i64_index("email_hash", &email_hash);
    assert!(results.is_some());
    assert_eq!(results.unwrap().len(), 1);
}

#[test]
fn test_uuid_index_queries() {
    // Create test products
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    
    let product1 = Product::new(user1.id, "Laptop".to_string());
    let product2 = Product::new(user1.id, "Mouse".to_string());
    let product3 = Product::new(user2.id, "Keyboard".to_string());
    
    let product_cache1 = ProductIndexCache::from_product(&product1);
    let product_cache2 = ProductIndexCache::from_product(&product2);
    let product_cache3 = ProductIndexCache::from_product(&product3);
    
    // Create cache
    let cache = IdxModelCache::new(vec![
        product_cache1.clone(),
        product_cache2.clone(),
        product_cache3.clone(),
    ]).unwrap();
    
    // Test get by user_id
    let results = cache.get_by_uuid_index("user_id", &user1.id);
    assert!(results.is_some());
    let user1_products = results.unwrap();
    assert_eq!(user1_products.len(), 2); // product1 and product2
    
    let results = cache.get_by_uuid_index("user_id", &user2.id);
    assert!(results.is_some());
    let user2_products = results.unwrap();
    assert_eq!(user2_products.len(), 1); // product3
}

#[test]
fn test_duplicate_primary_key_error() {
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user_cache1 = UserIndexCache::from_user(&user1);
    let user_cache1_duplicate = user_cache1.clone();
    
    // Try to create cache with duplicate primary key
    let result = IdxModelCache::new(vec![user_cache1, user_cache1_duplicate]);
    assert!(result.is_err());
}

#[test]
fn test_transaction_aware_cache_staging() {
    // Create shared cache
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user_cache1 = UserIndexCache::from_user(&user1);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache1.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Add a new user in transaction
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    let user_cache2 = UserIndexCache::from_user(&user2);
    tx_cache.add(user_cache2.clone());
    
    // User2 should be visible in transaction
    assert!(tx_cache.contains_primary(&user2.id));
    let retrieved = tx_cache.get_by_primary(&user2.id);
    assert!(retrieved.is_some());
    
    // User2 should NOT be visible in shared cache yet
    assert!(!shared_cache.read().contains_primary(&user2.id));
    
    // Update user1 in transaction
    let mut updated_user_cache1 = user_cache1.clone();
    updated_user_cache1.email_hash = 777777;
    tx_cache.update(updated_user_cache1.clone());
    
    // Updated user1 should be visible in transaction
    let retrieved = tx_cache.get_by_primary(&user1.id).unwrap();
    assert_eq!(retrieved.email_hash, 777777);
    
    // Original user1 should still be in shared cache
    let shared_retrieved = shared_cache.read().get_by_primary(&user1.id).unwrap();
    assert_ne!(shared_retrieved.email_hash, 777777);
}

#[test]
fn test_transaction_aware_cache_remove_staging() {
    // Create shared cache
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user_cache1 = UserIndexCache::from_user(&user1);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache1.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Remove user1 in transaction
    tx_cache.remove(&user1.id);
    
    // User1 should NOT be visible in transaction
    assert!(!tx_cache.contains_primary(&user1.id));
    assert!(tx_cache.get_by_primary(&user1.id).is_none());
    
    // User1 should still be in shared cache
    assert!(shared_cache.read().contains_primary(&user1.id));
}

#[tokio::test]
async fn test_transaction_aware_cache_commit() {
    // Create shared cache
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user_cache1 = UserIndexCache::from_user(&user1);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache1.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Add a new user in transaction
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    let user_cache2 = UserIndexCache::from_user(&user2);
    tx_cache.add(user_cache2.clone());
    
    // Update user1 in transaction
    let mut updated_user_cache1 = user_cache1.clone();
    updated_user_cache1.email_hash = 888888;
    tx_cache.update(updated_user_cache1.clone());
    
    // Verify staged changes are not in shared cache yet
    assert!(!shared_cache.read().contains_primary(&user2.id));
    assert_ne!(shared_cache.read().get_by_primary(&user1.id).unwrap().email_hash, 888888);
    
    // Commit transaction
    use postgres_index_cache::TransactionAware;
    tx_cache.on_commit().await.unwrap();
    
    // Verify changes are now in shared cache
    assert!(shared_cache.read().contains_primary(&user2.id));
    assert_eq!(shared_cache.read().get_by_primary(&user1.id).unwrap().email_hash, 888888);
    assert!(shared_cache.read().get_by_primary(&user2.id).is_some());
}

#[tokio::test]
async fn test_transaction_aware_cache_rollback() {
    // Create shared cache
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user_cache1 = UserIndexCache::from_user(&user1);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache1.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Add a new user in transaction
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    let user_cache2 = UserIndexCache::from_user(&user2);
    tx_cache.add(user_cache2.clone());
    
    // Update user1 in transaction
    let mut updated_user_cache1 = user_cache1.clone();
    updated_user_cache1.email_hash = 999999;
    tx_cache.update(updated_user_cache1.clone());
    
    // Remove operation
    tx_cache.remove(&user1.id);
    
    // Verify staged changes
    assert!(tx_cache.contains_primary(&user2.id));
    assert!(!tx_cache.contains_primary(&user1.id));
    
    // Rollback transaction
    use postgres_index_cache::TransactionAware;
    tx_cache.on_rollback().await.unwrap();
    
    // Verify shared cache is unchanged
    assert!(shared_cache.read().contains_primary(&user1.id));
    assert!(!shared_cache.read().contains_primary(&user2.id));
    assert_eq!(shared_cache.read().get_by_primary(&user1.id).unwrap().email_hash, user_cache1.email_hash);
    
    // Verify transaction cache now reflects shared cache
    assert!(tx_cache.get_by_primary(&user1.id).is_some());
    assert!(tx_cache.get_by_primary(&user2.id).is_none());
}

#[tokio::test]
async fn test_transaction_aware_cache_i64_index_with_staging() {
    // Create shared cache with initial users
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    let user2 = User::new("bob".to_string(), "bob@example.com".to_string());
    
    let user_cache1 = UserIndexCache::from_user(&user1);
    let user_cache2 = UserIndexCache::from_user(&user2);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache1.clone(), user_cache2.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Add a new user with same username as alice
    let user3 = User::new("alice".to_string(), "alice3@example.com".to_string());
    let user_cache3 = UserIndexCache::from_user(&user3);
    tx_cache.add(user_cache3.clone());
    
    // Query by username_hash within transaction
    let alice_hash = user_cache1.username_hash;
    let results = tx_cache.get_by_i64_index("username_hash", &alice_hash);
    
    // Should get both alice users (user1 from shared, user3 from staging)
    assert_eq!(results.len(), 2);
    
    // Update user2's username hash
    let mut updated_user_cache2 = user_cache2.clone();
    updated_user_cache2.username_hash = alice_hash; // Change bob's hash to alice's
    tx_cache.update(updated_user_cache2.clone());
    
    // Query again - should now get 3 results
    let results = tx_cache.get_by_i64_index("username_hash", &alice_hash);
    assert_eq!(results.len(), 3);
    
    // Rollback and verify shared cache is unchanged
    use postgres_index_cache::TransactionAware;
    tx_cache.on_rollback().await.unwrap();
    
    let shared_guard = shared_cache.read();
    let shared_results = shared_guard.get_by_i64_index("username_hash", &alice_hash).unwrap();
    assert_eq!(shared_results.len(), 1); // Only original alice
}

#[tokio::test]
async fn test_transaction_aware_cache_uuid_index_with_staging() {
    // Create shared cache with initial products
    let user1 = User::new("alice".to_string(), "alice@example.com".to_string());
    
    let product1 = Product::new(user1.id, "Laptop".to_string());
    let product_cache1 = ProductIndexCache::from_product(&product1);
    
    let shared_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![product_cache1.clone()]).unwrap()
    ));
    
    // Create transaction-aware cache
    let tx_cache = TransactionAwareIdxModelCache::new(shared_cache.clone());
    
    // Add new products for same user
    let product2 = Product::new(user1.id, "Mouse".to_string());
    let product3 = Product::new(user1.id, "Keyboard".to_string());
    
    let product_cache2 = ProductIndexCache::from_product(&product2);
    let product_cache3 = ProductIndexCache::from_product(&product3);
    
    tx_cache.add(product_cache2.clone());
    tx_cache.add(product_cache3.clone());
    
    // Query by user_id within transaction
    let results = tx_cache.get_by_uuid_index("user_id", &user1.id);
    assert_eq!(results.len(), 3); // All three products
    
    // Commit and verify
    use postgres_index_cache::TransactionAware;
    tx_cache.on_commit().await.unwrap();
    
    let shared_guard = shared_cache.read();
    let shared_results = shared_guard.get_by_uuid_index("user_id", &user1.id).unwrap();
    assert_eq!(shared_results.len(), 3);
}