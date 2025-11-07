mod common;

use std::sync::Arc;
use parking_lot::RwLock;
use postgres_index_cache::{
    CacheNotification, CacheNotificationListener, IdxModelCache, IndexCacheHandler,
};
use serde_json::json;
use uuid::Uuid;

use common::entities::{User, UserIndexCache, Product, ProductIndexCache};

#[tokio::test]
async fn test_user_cache_notification_insert() {
    // Create empty user cache
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
    // Create handler for users table
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    // Create listener and register handler
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Create a test user
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    // Create notification payload for insert
    let notification = CacheNotification {
        table: "users".to_string(),
        action: "insert".to_string(),
        id: user_id,
        data: Some(json!({
            "id": user_id.to_string(),
            "username": "alice",
            "email": "alice@example.com"
        })),
    };
    
    // Convert the user to cache entry manually (simulating what would be in the notification)
    let user_cache_entry = UserIndexCache::from_user(&user);
    let notification_with_cache = CacheNotification {
        table: "users".to_string(),
        action: "insert".to_string(),
        id: user_id,
        data: Some(serde_json::to_value(&user_cache_entry).unwrap()),
    };
    
    let payload = serde_json::to_string(&notification_with_cache).unwrap();
    
    // Process the notification
    listener.process_notification(&payload).await;
    
    // Verify the cache was updated
    let cache = user_cache.read();
    assert!(cache.contains_primary(&user_id));
    
    let cached_user = cache.get_by_primary(&user_id).unwrap();
    assert_eq!(cached_user.id, user_id);
}

#[tokio::test]
async fn test_user_cache_notification_update() {
    // Create user cache with initial data
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let user_cache_entry = UserIndexCache::from_user(&user);
    
    let user_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache_entry.clone()]).unwrap()
    ));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Create updated user
    let updated_user = User {
        id: user_id,
        username: "alice".to_string(),
        email: "alice.updated@example.com".to_string(),
    };
    let updated_cache_entry = UserIndexCache::from_user(&updated_user);
    
    // Create notification for update
    let notification = CacheNotification {
        table: "users".to_string(),
        action: "update".to_string(),
        id: user_id,
        data: Some(serde_json::to_value(&updated_cache_entry).unwrap()),
    };
    
    let payload = serde_json::to_string(&notification).unwrap();
    
    // Process the notification
    listener.process_notification(&payload).await;
    
    // Verify the cache was updated
    let cache = user_cache.read();
    let cached_user = cache.get_by_primary(&user_id).unwrap();
    assert_eq!(cached_user.email_hash, updated_cache_entry.email_hash);
}

#[tokio::test]
async fn test_user_cache_notification_delete() {
    // Create user cache with initial data
    let user_id = Uuid::new_v4();
    let user = User {
        id: user_id,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let user_cache_entry = UserIndexCache::from_user(&user);
    
    let user_cache = Arc::new(RwLock::new(
        IdxModelCache::new(vec![user_cache_entry]).unwrap()
    ));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Verify user exists before deletion
    assert!(user_cache.read().contains_primary(&user_id));
    
    // Create notification for delete
    let notification = CacheNotification {
        table: "users".to_string(),
        action: "delete".to_string(),
        id: user_id,
        data: None,
    };
    
    let payload = serde_json::to_string(&notification).unwrap();
    
    // Process the notification
    listener.process_notification(&payload).await;
    
    // Verify the cache entry was removed
    let cache = user_cache.read();
    assert!(!cache.contains_primary(&user_id));
}

#[tokio::test]
async fn test_product_cache_notification_insert() {
    // Create empty product cache
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
    // Create handler for products table
    let handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    // Create listener and register handler
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Create a test product
    let product_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let product = Product {
        id: product_id,
        user_id,
        product_name: "Widget".to_string(),
    };
    
    let product_cache_entry = ProductIndexCache::from_product(&product);
    
    // Create notification payload for insert
    let notification = CacheNotification {
        table: "products".to_string(),
        action: "insert".to_string(),
        id: product_id,
        data: Some(serde_json::to_value(&product_cache_entry).unwrap()),
    };
    
    let payload = serde_json::to_string(&notification).unwrap();
    
    // Process the notification
    listener.process_notification(&payload).await;
    
    // Verify the cache was updated
    let cache = product_cache.read();
    assert!(cache.contains_primary(&product_id));
    
    let cached_product = cache.get_by_primary(&product_id).unwrap();
    assert_eq!(cached_product.id, product_id);
    assert_eq!(cached_product.user_id, user_id);
}

#[tokio::test]
async fn test_multi_table_listener() {
    // Create caches for both users and products
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
    // Create handlers
    let user_handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    let product_handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    // Create listener and register both handlers
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(user_handler);
    listener.register_handler(product_handler);
    
    // Create test data
    let user_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    
    let user = User {
        id: user_id,
        username: "bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    let user_cache_entry = UserIndexCache::from_user(&user);
    
    let product = Product {
        id: product_id,
        user_id,
        product_name: "Gadget".to_string(),
    };
    let product_cache_entry = ProductIndexCache::from_product(&product);
    
    // Process user notification
    let user_notification = CacheNotification {
        table: "users".to_string(),
        action: "insert".to_string(),
        id: user_id,
        data: Some(serde_json::to_value(&user_cache_entry).unwrap()),
    };
    listener.process_notification(&serde_json::to_string(&user_notification).unwrap()).await;
    
    // Process product notification
    let product_notification = CacheNotification {
        table: "products".to_string(),
        action: "insert".to_string(),
        id: product_id,
        data: Some(serde_json::to_value(&product_cache_entry).unwrap()),
    };
    listener.process_notification(&serde_json::to_string(&product_notification).unwrap()).await;
    
    // Verify both caches were updated
    assert!(user_cache.read().contains_primary(&user_id));
    assert!(product_cache.read().contains_primary(&product_id));
    
    // Verify the product's user_id index works
    let product_cache_read = product_cache.read();
    let products_by_user = product_cache_read.get_by_uuid_index("user_id", &user_id).unwrap();
    assert_eq!(products_by_user.len(), 1);
    assert_eq!(products_by_user[0], product_id);
}

#[tokio::test]
async fn test_notification_with_unknown_table() {
    // Create listener without any handlers
    let listener = CacheNotificationListener::new();
    
    // Create notification for unknown table
    let notification = CacheNotification {
        table: "unknown_table".to_string(),
        action: "insert".to_string(),
        id: Uuid::new_v4(),
        data: None,
    };
    
    let payload = serde_json::to_string(&notification).unwrap();
    
    // This should not panic, just log a debug message
    listener.process_notification(&payload).await;
    // Test passes if no panic occurs
}

#[test]
fn test_custom_channel_name() {
    let listener = CacheNotificationListener::with_channel("my_custom_channel".to_string());
    assert_eq!(listener.channel(), "my_custom_channel");
}