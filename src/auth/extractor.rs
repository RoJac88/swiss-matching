use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
};

use crate::errors::AppError;

use super::jwt::{Claims, validate_token};

#[derive(Clone)]
pub struct CurrentUser(pub Claims);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::InvalidAuthHeader)?;

        let bearer = "Bearer ";
        if !auth_header.starts_with(bearer) {
            return Err(AppError::InvalidAuthHeader);
        }

        let token = &auth_header[bearer.len()..];

        let claims = validate_token(token).map_err(|_| AppError::TokenInvalid)?;

        Ok(CurrentUser(claims))
    }
}

pub async fn require_admin(
    CurrentUser(claims): CurrentUser,
) -> Result<(), (StatusCode, &'static str)> {
    if claims.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Admin access required"));
    }
    Ok(())
}
