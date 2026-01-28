use std::env;

use sqlx::Sqlite;

use crate::{auth::hasher::hash_password, repositories::auth_repo::create_admin};

pub async fn create_administrator(pool: &sqlx::Pool<Sqlite>) {
    let username = env::var("ADMIN_USERNAME");
    let password = env::var("ADMIN_PASSWORD");
    if let (Ok(name), Ok(pass)) = (username, password) {
        let password_hash = hash_password(&pass).expect("Failed to hash admin password");
        create_admin(pool, &name, &password_hash)
            .await
            .expect("failed to create admin user");
        tracing::info!("Created admin user: {}", name);
    }
}
