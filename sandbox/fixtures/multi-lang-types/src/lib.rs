// Multi-language types fixture
// Rust library: domain models with rich type relationships
// Tested by type-ref walkers across Python, TypeScript, and Go bindings

use std::collections::HashMap;
use std::sync::Arc;

/// Represents a user in the system
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub email: Email,
    pub name: String,
    pub role: UserRole,
    pub metadata: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

#[derive(Debug, Clone)]
pub struct Email(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    Editor,
    Viewer,
    Guest,
}

/// Repository trait - defines the contract for data access
pub trait Repository<T> {
    fn find_by_id(&self, id: &str) -> Result<Option<T>, RepositoryError>;
    fn find_all(&self) -> Result<Vec<T>, RepositoryError>;
    fn save(&self, entity: &T) -> Result<(), RepositoryError>;
    fn delete(&self, id: &str) -> Result<(), RepositoryError>;
}

/// Errors that can occur in repository operations
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Concurrent modification: entity was modified by another process")]
    ConcurrentModification,
}

/// PostgreSQL implementation of the Repository trait
pub struct PostgresRepository {
    connection_string: String,
}

impl PostgresRepository {
    pub fn new(connection_string: String) -> Self {
        Self { connection_string }
    }

    pub fn with_pool(pool: Arc<sqlx::PgPool>) -> Self {
        Self {
            connection_string: pool.to_string(),
        }
    }

    async fn execute_query(&self, query: &str) -> Result<sqlx::PgRow, sqlx::Error> {
        sqlx::query(query).fetch_one(&*self.pool_ref()).await
    }

    fn pool_ref(&self) -> Arc<sqlx::PgPool> {
        Arc::new(sqlx::PgPool::connect(&self.connection_string).unwrap())
    }
}

impl Repository<User> for PostgresRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        let query = "SELECT id, email, name, role, metadata, created_at FROM users WHERE id = $1";
        self.execute_query(query)
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(None)
    }

    fn find_all(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(Vec::new())
    }

    fn save(&self, user: &User) -> Result<(), RepositoryError> {
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), RepositoryError> {
        Ok(())
    }
}

/// Service layer - orchestrates business logic
pub struct UserService<R: Repository<User>> {
    repository: Arc<R>,
    email_sender: Arc<dyn EmailSender>,
}

impl<R: Repository<User>> UserService<R> {
    pub fn new(repository: Arc<R>, email_sender: Arc<dyn EmailSender>) -> Self {
        Self { repository, email_sender }
    }

    pub async fn create_user(&self, email: Email, name: String) -> Result<User, UserServiceError> {
        if name.is_empty() {
            return Err(UserServiceError::ValidationError("Name cannot be empty".into()));
        }

        let user = User {
            id: UserId(uuid::Uuid::new_v4().to_string()),
            email,
            name,
            role: UserRole::Viewer,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now(),
        };

        self.repository.save(&user).map_err(|e| UserServiceError::RepositoryError(e))?;
        Ok(user)
    }

    pub fn find_user(&self, id: &str) -> Result<Option<User>, UserServiceError> {
        self.repository
            .find_by_id(id)
            .map_err(|e| UserServiceError::RepositoryError(e))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UserServiceError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),

    #[error("Email error: {0}")]
    EmailError(String),
}

/// Email sender trait
pub trait EmailSender: Send + Sync {
    fn send(&self, to: &Email, subject: &str, body: &str) -> Result<(), EmailError>;
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("SMTP error: {0}")]
    SmtpError(String),

    #[error("Invalid recipient: {0}")]
    InvalidRecipient(String),
}

/// Pagination for list operations
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total: u64, page: u32, page_size: u32) -> Self {
        Self { items, total, page, page_size }
    }

    pub fn total_pages(&self) -> u32 {
        (self.total + self.page_size as u64 - 1) / self.page_size as u64
    }

    pub fn has_next(&self) -> bool {
        self.page < self.total_pages()
    }

    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}
