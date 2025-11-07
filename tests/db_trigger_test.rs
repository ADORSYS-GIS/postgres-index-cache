mod common;

use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use postgres_index_cache::{
    CacheNotificationListener, IdxModelCache, IndexCacheHandler,
};
use sqlx::PgPool;
use tokio::time::sleep;

use common::{
    User, Product, UserIndexCache, ProductIndexCache,
    UserRepository, ProductRepository,
};

/// Helper function to get database URL from environment or use default
fn get_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/test_db".to_string())
}

/// Setup the database connection pool and create tables with triggers
async fn setup_database() -> PgPool {
    let pool = PgPool::connect(&get_database_url())
        .await
        .expect("Failed to connect to database");

    // Read and execute the SQL script that creates tables and triggers
    let sql_script = include_str!("../sql/cache_notification_triggers.sql");
    
    // Execute the script (split by statement if needed)
    sqlx::query(sql_script)
        .execute(&pool)
        .await
        .expect("Failed to execute trigger setup script");

    pool
}

/// Clean up database after tests
async fn cleanup_database(pool: &PgPool) {
    // Drop triggers first
    sqlx::query("DROP TRIGGER IF EXISTS users_cache_notify ON users")
        .execute(pool)
        .await
        .ok();
    
    sqlx::query("DROP TRIGGER IF EXISTS products_cache_notify ON products")
        .execute(pool)
        .await
        .ok();

    // Drop tables
    sqlx::query("DROP TABLE IF EXISTS products CASCADE")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DROP TABLE IF EXISTS users CASCADE")
        .execute(pool)
        .await
        .ok();

    // Drop function
    sqlx::query("DROP FUNCTION IF EXISTS notify_cache_change()")
        .execute(pool)
        .await
        .ok();
}

#[tokio::test]
#[serial_test::serial]
async fn test_user_insert_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // Create empty user cache
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
    // Create handler for users table
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    // Create listener and register handler
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening to notifications in background
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    // Give listener time to start
    sleep(Duration::from_millis(100)).await;
    
    // Create repository and insert a user directly into the database
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("alice".to_string(), "alice@example.com".to_string());
    
    user_repo.create(&user).await.expect("Failed to create user");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache was updated via the trigger
    let cache = user_cache.read();
    assert!(cache.contains_primary(&user.id), "User should be in cache after insert");
    
    let cached_user = cache.get_by_primary(&user.id);
    assert!(cached_user.is_some(), "User should be retrievable from cache");
    
    // Verify the cached data matches
    let cached_user = cached_user.unwrap();
    assert_eq!(cached_user.id, user.id);
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_product_insert_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // First, insert a user (products reference users)
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("bob".to_string(), "bob@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    // Create empty product cache
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
    // Create handler for products table
    let handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    // Create listener and register handler
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening to notifications in background
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    // Give listener time to start
    sleep(Duration::from_millis(100)).await;
    
    // Create repository and insert a product directly into the database
    let product_repo = ProductRepository::new(pool.clone());
    let product = Product::new(user.id, "Laptop".to_string());
    
    product_repo.create(&product).await.expect("Failed to create product");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache was updated via the trigger
    let cache = product_cache.read();
    assert!(cache.contains_primary(&product.id), "Product should be in cache after insert");
    
    let cached_product = cache.get_by_primary(&product.id);
    assert!(cached_product.is_some(), "Product should be retrievable from cache");
    
    // Verify the cached data matches
    let cached_product = cached_product.unwrap();
    assert_eq!(cached_product.id, product.id);
    assert_eq!(cached_product.user_id, user.id);
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_user_update_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // Create user cache with initial user
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("charlie".to_string(), "charlie@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    let initial_cache = UserIndexCache::from_user(&user);
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![initial_cache.clone()]).unwrap()));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Update the user in the database
    let mut updated_user = user.clone();
    updated_user.email = "charlie.updated@example.com".to_string();
    user_repo.update(&updated_user).await.expect("Failed to update user");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache was updated
    let cache = user_cache.read();
    let cached_user = cache.get_by_primary(&user.id).expect("User should still be in cache");
    
    // The email hash should have changed
    let updated_cache = UserIndexCache::from_user(&updated_user);
    assert_eq!(cached_user.email_hash, updated_cache.email_hash, "Email hash should be updated in cache");
    assert_ne!(cached_user.email_hash, initial_cache.email_hash, "Email hash should differ from initial");
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_product_update_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // Create user and product
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("diana".to_string(), "diana@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    let product_repo = ProductRepository::new(pool.clone());
    let product = Product::new(user.id, "Mouse".to_string());
    product_repo.create(&product).await.expect("Failed to create product");
    
    let initial_cache = ProductIndexCache::from_product(&product);
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![initial_cache.clone()]).unwrap()));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Update the product in the database
    let mut updated_product = product.clone();
    updated_product.product_name = "Wireless Mouse".to_string();
    product_repo.update(&updated_product).await.expect("Failed to update product");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache was updated
    let cache = product_cache.read();
    let cached_product = cache.get_by_primary(&product.id).expect("Product should still be in cache");
    
    // The product name hash should have changed
    let updated_cache = ProductIndexCache::from_product(&updated_product);
    assert_eq!(cached_product.product_name_hash, updated_cache.product_name_hash, 
               "Product name hash should be updated in cache");
    assert_ne!(cached_product.product_name_hash, initial_cache.product_name_hash, 
               "Product name hash should differ from initial");
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_user_delete_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // Create user
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("eve".to_string(), "eve@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    let initial_cache = UserIndexCache::from_user(&user);
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![initial_cache]).unwrap()));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "users".to_string(),
        user_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Verify user is in cache
    assert!(user_cache.read().contains_primary(&user.id), "User should be in cache initially");
    
    // Delete the user from the database
    user_repo.delete(user.id).await.expect("Failed to delete user");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache entry was removed
    let cache = user_cache.read();
    assert!(!cache.contains_primary(&user.id), "User should be removed from cache after delete");
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_product_delete_triggers_cache_notification() {
    // Setup database
    let pool = setup_database().await;
    
    // Create user and product
    let user_repo = UserRepository::new(pool.clone());
    let user = User::new("frank".to_string(), "frank@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    let product_repo = ProductRepository::new(pool.clone());
    let product = Product::new(user.id, "Keyboard".to_string());
    product_repo.create(&product).await.expect("Failed to create product");
    
    let initial_cache = ProductIndexCache::from_product(&product);
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![initial_cache]).unwrap()));
    
    // Create handler and listener
    let handler = Arc::new(IndexCacheHandler::new(
        "products".to_string(),
        product_cache.clone(),
    ));
    
    let mut listener = CacheNotificationListener::new();
    listener.register_handler(handler);
    
    // Start listening
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Verify product is in cache
    assert!(product_cache.read().contains_primary(&product.id), "Product should be in cache initially");
    
    // Delete the product from the database
    product_repo.delete(product.id).await.expect("Failed to delete product");
    
    // Give time for notification to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Verify the cache entry was removed
    let cache = product_cache.read();
    assert!(!cache.contains_primary(&product.id), "Product should be removed from cache after delete");
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_multi_table_operations_with_cache() {
    // Setup database
    let pool = setup_database().await;
    
    // Create caches for both tables
    let user_cache: Arc<RwLock<IdxModelCache<UserIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    let product_cache: Arc<RwLock<IdxModelCache<ProductIndexCache>>> = 
        Arc::new(RwLock::new(IdxModelCache::new(vec![]).unwrap()));
    
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
    
    // Start listening
    let pool_clone = pool.clone();
    let _listen_handle = tokio::spawn(async move {
        listener.listen(&pool_clone).await.ok();
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // Create repositories
    let user_repo = UserRepository::new(pool.clone());
    let product_repo = ProductRepository::new(pool.clone());
    
    // Insert a user
    let user = User::new("grace".to_string(), "grace@example.com".to_string());
    user_repo.create(&user).await.expect("Failed to create user");
    
    sleep(Duration::from_millis(500)).await;
    
    // Verify user is in cache
    assert!(user_cache.read().contains_primary(&user.id), "User should be in cache");
    
    // Insert a product for that user
    let product = Product::new(user.id, "Monitor".to_string());
    product_repo.create(&product).await.expect("Failed to create product");
    
    sleep(Duration::from_millis(500)).await;
    
    // Verify product is in cache
    assert!(product_cache.read().contains_primary(&product.id), "Product should be in cache");
    
    // Verify the product's user_id index
    let product_cache_read = product_cache.read();
    let products_by_user = product_cache_read.get_by_uuid_index("user_id", &user.id);
    assert!(products_by_user.is_some(), "Should be able to query products by user_id");
    assert_eq!(products_by_user.unwrap().len(), 1, "Should have 1 product for this user");
    
    // Cleanup
    cleanup_database(&pool).await;
    pool.close().await;
}