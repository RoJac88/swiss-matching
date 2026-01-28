use crate::{
    AppState,
    auth::extractor::CurrentUser,
    errors::AppError,
    payloads::NewPlayer,
    repositories::player_repo,
    responses::{AppResponse, Json, SuccessResponse},
    services::player_service::{self, check_fide_player_exists},
};
use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use sqlx::SqlitePool;

async fn create_player(
    State(pool): State<SqlitePool>,
    CurrentUser(_): CurrentUser,
    Json(payload): Json<NewPlayer>,
) -> impl IntoResponse {
    let id = match player_repo::create_player(&pool, payload)
        .await
        .map_err(|e| Into::<AppError>::into(e))
    {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };
    AppResponse::Success {
        payload: SuccessResponse::PlayerCreated { id },
    }
    .into_response()
}

async fn list_players(State(pool): State<SqlitePool>) -> impl IntoResponse {
    let players = match player_repo::list_players(&pool)
        .await
        .map_err(|e| Into::<AppError>::into(e))
    {
        Ok(list) => list,
        Err(e) => return e.into_response(),
    };
    AppResponse::Success {
        payload: SuccessResponse::PlayerList { players },
    }
    .into_response()
}

async fn get_fide_player(
    Path(fide_id): Path<i64>,
    State(pool): State<sqlx::Pool<sqlx::Sqlite>>,
    State(client): State<reqwest::Client>,
) -> impl IntoResponse {
    match check_fide_player_exists(&pool, fide_id, &client).await {
        Ok(Some(player_service::FidePlayerCheck::Exists(id))) => AppResponse::Success {
            payload: SuccessResponse::PlayerExists { id, fide_id },
        }
        .into_response(),
        Ok(Some(player_service::FidePlayerCheck::Updated(player))) => AppResponse::Success {
            payload: SuccessResponse::PlayerUpdated { player },
        }
        .into_response(),
        Err(e) => e.into_response(),
        Ok(None) => match player_service::scrape_fide_player(&client, fide_id).await {
            Ok(player) => Into::<AppResponse>::into(player).into_response(),
            Err(e) => e.into_response(),
        },
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_player))
        .route("/", get(list_players))
        .route("/fide/{fide_id}", get(get_fide_player))
        .with_state(state)
}
