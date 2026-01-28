use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{Router, extract::State, response::IntoResponse, routing::post};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::{hasher::hash_password, jwt::create_token},
    errors::AppError,
    payloads::{LoginPayload, NewUser},
    repositories::auth_repo::{self, get_user},
    responses::{AppResponse, Json, SuccessResponse},
};

async fn login(
    State(pool): State<SqlitePool>,
    Json(payload): Json<LoginPayload>,
) -> impl IntoResponse {
    let user = match get_user(&pool, &payload.username).await {
        Ok(user) => user,
        Err(e) => return e.into_response(),
    };
    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(hash) => hash,
        Err(_) => return AppError::Unknown.into_response(),
    };
    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return AppError::LoginFailed("Invalid credentials".to_string()).into_response();
    }
    let token = match create_token(user.id, &user.role, chrono::Duration::hours(24)) {
        Ok(t) => t,
        Err(_) => return AppError::Unknown.into_response(),
    };
    AppResponse::Success {
        payload: SuccessResponse::LoginSuccess {
            token,
            role: user.role.to_string(),
        },
    }
    .into_response()
}

async fn create_user(
    State(pool): State<SqlitePool>,
    Json(payload): Json<NewUser>,
) -> impl IntoResponse {
    match get_user(&pool, &payload.username).await {
        Err(AppError::LoginFailed(_)) => {
            let password_hash = match hash_password(&payload.password) {
                Ok(hash) => hash,
                Err(e) => return e.into_response(),
            };
            match auth_repo::create_user(&pool, &payload.username, &password_hash, payload.email)
                .await
            {
                Ok(id) => AppResponse::Success {
                    payload: SuccessResponse::UserCreated { id },
                }
                .into_response(),
                Err(_) => AppError::Unknown.into_response(),
            }
        }
        Ok(_) => return AppError::UsernameTaken(payload.username).into_response(),
        _ => return AppError::Unknown.into_response(),
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(create_user))
        .with_state(state)
}
