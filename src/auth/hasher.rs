use crate::errors::AppError;
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::default(); // uses Argon2id v19 with secure defaults (m=19MiB, t=2, p=1)

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("hash_password: {:?}", e);
            AppError::Unknown
        })?
        .to_string();

    Ok(password_hash)
}
