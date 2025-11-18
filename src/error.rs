use postgres_unit_of_work::TransactionError;

/// Error type for cache operations
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Duplicate primary key: {0}")]
    DuplicatePrimaryKey(String),
    
    #[error("Transaction commit failed: {0}")]
    CommitFailed(String),
    
    #[error("Transaction rollback failed: {0}")]
    RollbackFailed(String),
    
    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
}

/// Result type for cache operations
pub type CacheResult<T> = Result<T, CacheError>;

/// Conversion from CacheError to TransactionError
impl From<CacheError> for TransactionError {
    fn from(err: CacheError) -> Self {
        match err {
            CacheError::CommitFailed(msg) => TransactionError::CommitFailed(msg),
            CacheError::RollbackFailed(msg) => TransactionError::RollbackFailed(msg),
            CacheError::DuplicatePrimaryKey(msg) | CacheError::OperationFailed(msg) => {
                TransactionError::CommitFailed(format!("Cache error: {msg}"))
            }
        }
    }
}