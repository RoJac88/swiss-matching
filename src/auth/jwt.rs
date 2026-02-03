use std::{env, sync::LazyLock};

use chrono::{Duration, Utc};
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::Error as JwtError,
};
use serde::{Deserialize, Serialize};

static JWT_SECRET: LazyLock<String> = LazyLock::new(|| env::var("JWT_SECRET").unwrap());

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: u32,
    pub username: String,
    pub role: String,
    pub exp: i64,
}

pub fn create_token(
    user_id: u32,
    username: String,
    role: String,
    duration: Duration,
) -> Result<String, JwtError> {
    let claims = Claims {
        sub: user_id,
        username,
        role,
        exp: (Utc::now() + duration).timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
}

pub fn validate_token(token: &str) -> Result<Claims, JwtError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}
