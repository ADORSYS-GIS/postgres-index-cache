use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::entities::{User, Product, UserIndexCache, ProductIndexCache};

/// Repository for direct database access to users table
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user: &User) -> Result<(), sqlx::Error> {
        // Insert into users table
        sqlx::query(
            "INSERT INTO users (id, username, email) VALUES ($1, $2, $3)"
        )
        .bind(user.id)
        .bind(&user.username)
        .bind(&user.email)
        .execute(&self.pool)
        .await?;

        // Insert into user_index_cache table to trigger notification
        let cache = UserIndexCache::from_user(user);
        sqlx::query(
            "INSERT INTO user_index_cache (id, username_hash, email_hash) VALUES ($1, $2, $3)"
        )
        .bind(cache.id)
        .bind(cache.username_hash)
        .bind(cache.email_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update(&self, user: &User) -> Result<(), sqlx::Error> {
        // Update users table
        sqlx::query(
            "UPDATE users SET username = $2, email = $3 WHERE id = $1"
        )
        .bind(user.id)
        .bind(&user.username)
        .bind(&user.email)
        .execute(&self.pool)
        .await?;

        // Update user_index_cache table to trigger notification
        let cache = UserIndexCache::from_user(user);
        sqlx::query(
            "UPDATE user_index_cache SET username_hash = $2, email_hash = $3 WHERE id = $1"
        )
        .bind(cache.id)
        .bind(cache.username_hash)
        .bind(cache.email_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), sqlx::Error> {
        // Delete from user_index_cache first to trigger notification
        sqlx::query("DELETE FROM user_index_cache WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        // Then delete from users table
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, username, email FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| User {
            id: r.get("id"),
            username: r.get("username"),
            email: r.get("email"),
        }))
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("count"))
    }
}

/// Repository for direct database access to products table
pub struct ProductRepository {
    pool: PgPool,
}

impl ProductRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, product: &Product) -> Result<(), sqlx::Error> {
        // Insert into products table
        sqlx::query(
            "INSERT INTO products (id, user_id, product_name) VALUES ($1, $2, $3)"
        )
        .bind(product.id)
        .bind(product.user_id)
        .bind(&product.product_name)
        .execute(&self.pool)
        .await?;

        // Insert into product_index_cache table to trigger notification
        let cache = ProductIndexCache::from_product(product);
        sqlx::query(
            "INSERT INTO product_index_cache (id, user_id, product_name_hash) VALUES ($1, $2, $3)"
        )
        .bind(cache.id)
        .bind(cache.user_id)
        .bind(cache.product_name_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update(&self, product: &Product) -> Result<(), sqlx::Error> {
        // Update products table
        sqlx::query(
            "UPDATE products SET user_id = $2, product_name = $3 WHERE id = $1"
        )
        .bind(product.id)
        .bind(product.user_id)
        .bind(&product.product_name)
        .execute(&self.pool)
        .await?;

        // Update product_index_cache table to trigger notification
        let cache = ProductIndexCache::from_product(product);
        sqlx::query(
            "UPDATE product_index_cache SET user_id = $2, product_name_hash = $3 WHERE id = $1"
        )
        .bind(cache.id)
        .bind(cache.user_id)
        .bind(cache.product_name_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), sqlx::Error> {
        // Delete from product_index_cache first to trigger notification
        sqlx::query("DELETE FROM product_index_cache WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        // Then delete from products table (will cascade due to foreign key)
        sqlx::query("DELETE FROM products WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Product>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, user_id, product_name FROM products WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Product {
            id: r.get("id"),
            user_id: r.get("user_id"),
            product_name: r.get("product_name"),
        }))
    }

    #[allow(dead_code)]
    pub async fn find_by_user_id(&self, user_id: Uuid) -> Result<Vec<Product>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, user_id, product_name FROM products WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| Product {
            id: r.get("id"),
            user_id: r.get("user_id"),
            product_name: r.get("product_name"),
        }).collect())
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM products")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("count"))
    }
}