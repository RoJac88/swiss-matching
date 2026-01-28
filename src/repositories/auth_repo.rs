use sqlx::FromRow;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct DbUser {
    pub id: u32,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: u32,
    pub email: Option<String>,
}

pub async fn get_user(pool: &sqlx::SqlitePool, username: &str) -> Result<DbUser, AppError> {
    let maybe_user: Option<DbUser> = sqlx::query_as("select * from users where username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("get_user: {:?}", e);
            AppError::Unknown
        })?;
    if let Some(user) = maybe_user {
        Ok(user)
    } else {
        Err(AppError::LoginFailed("Invalid credentials".to_string()))
    }
}

pub async fn create_user(
    pool: &sqlx::SqlitePool,
    username: &str,
    password_hash: &str,
    email: Option<String>,
) -> sqlx::Result<i64> {
    let result =
        sqlx::query("insert into users (username, password_hash, email, role) values (?, ?, ?, ?)")
            .bind(username)
            .bind(password_hash)
            .bind(email)
            .bind("standard")
            .execute(pool)
            .await?;
    Ok(result.last_insert_rowid())
}

pub async fn create_admin(
    pool: &sqlx::SqlitePool,
    username: &str,
    password_hash: &str,
) -> sqlx::Result<i64> {
    let result =
        sqlx::query("insert or ignore into users (username, password_hash, role) values (?, ?, ?)")
            .bind(username)
            .bind(password_hash)
            .bind("admin")
            .execute(pool)
            .await?;
    Ok(result.last_insert_rowid())
}
