//! Database initialization and cleanup utilities for cache notification triggers
//!
//! This module provides functions to initialize and cleanup the PostgreSQL
//! cache notification trigger infrastructure required by postgres-index-cache.

use sqlx::PgPool;

/// Initialize the cache notification trigger function in the database
///
/// This function creates the `notify_cache_change()` PostgreSQL function
/// that can be used by triggers to send cache invalidation notifications.
///
/// # Example
///
/// ```rust,no_run
/// use sqlx::PgPool;
/// use postgres_index_cache::init_cache_triggers;
///
/// # async fn example(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// init_cache_triggers(pool).await?;
/// # Ok(())
/// # }
/// ```
pub async fn init_cache_triggers(pool: &PgPool) -> Result<(), sqlx::Error> {
    const SQL: &str = include_str!("../sql/cache_notification_triggers.sql");
    sqlx::raw_sql(SQL).execute(pool).await?;
    Ok(())
}

/// Cleanup the cache notification trigger function from the database
///
/// This function removes the `notify_cache_change()` PostgreSQL function
/// and all associated triggers that use it.
///
/// # Example
///
/// ```rust,no_run
/// use sqlx::PgPool;
/// use postgres_index_cache::cleanup_cache_triggers;
///
/// # async fn example(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// cleanup_cache_triggers(pool).await?;
/// # Ok(())
/// # }
/// ```
pub async fn cleanup_cache_triggers(pool: &PgPool) -> Result<(), sqlx::Error> {
    const SQL: &str = include_str!("../cleanup/cleanup_cache_notification_triggers.sql");
    sqlx::raw_sql(SQL).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires a running PostgreSQL instance
    async fn test_init_and_cleanup() -> Result<(), Box<dyn std::error::Error>> {
        let pool = PgPool::connect("postgresql://postgres:postgres@localhost:5432/test_db").await?;
        
        // Test initialization
        init_cache_triggers(&pool).await?;
        
        // Test cleanup
        cleanup_cache_triggers(&pool).await?;
        
        Ok(())
    }
}