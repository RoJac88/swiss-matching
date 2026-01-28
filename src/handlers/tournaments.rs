use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::extractor::CurrentUser,
    errors::AppError,
    models::tournament::Tournament,
    payloads::{NewRegistration, NewTournament, NextPairings, PlayerStatusPayload, RoundResult},
    responses::{AppResponse, Json, SuccessResponse},
    services::tournament_service,
};

async fn register_player(
    State(pool): State<SqlitePool>,
    Path(id): Path<u32>,
    CurrentUser(claims): CurrentUser,
    Json(payload): Json<NewRegistration>,
) -> impl IntoResponse {
    match tournament_service::register_player(&pool, id, claims, payload).await {
        Ok(id) => AppResponse::Success {
            payload: SuccessResponse::PlayerRegistered { id: id },
        }
        .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn create_tournament(
    State(pool): State<SqlitePool>,
    CurrentUser(claims): CurrentUser,
    Json(payload): Json<NewTournament>,
) -> impl IntoResponse {
    match tournament_service::create_tournament(&pool, claims.sub, payload).await {
        Ok(id) => AppResponse::Success {
            payload: SuccessResponse::TournamentCreated { id },
        }
        .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn generate_next_round_pairings(
    State(pool): State<SqlitePool>,
    Path(id): Path<u32>,
    CurrentUser(claims): CurrentUser,
    Json(payload): Json<NextPairings>,
) -> impl IntoResponse {
    match tournament_service::generate_next_pairings(&pool, id, claims, payload).await {
        Ok(pairings) => match pairings.commit(&pool).await {
            Ok(_) => Into::<AppResponse>::into(pairings).into_response(),
            Err(e) => Into::<AppError>::into(e).into_response(),
        },
        Err(e) => e.into_response(),
    }
}

async fn get_tournament(Path(id): Path<u32>, State(pool): State<SqlitePool>) -> impl IntoResponse {
    match tournament_service::read_tournament(&pool, id).await {
        Ok(tdata) => {
            let tournament: Tournament = tdata.into();
            let response: AppResponse = tournament.into();
            response.into_response()
        }
        Err(e) => Into::<AppError>::into(e).into_response(),
    }
}

async fn list_tournaments(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match tournament_service::list_tournaments(&pool).await {
        Ok(tournaments) => Into::<AppResponse>::into(tournaments).into_response(),
        Err(e) => Into::<AppError>::into(e).into_response(),
    }
}

async fn update_game_result(
    State(pool): State<SqlitePool>,
    Path(id): Path<u32>,
    CurrentUser(claims): CurrentUser,
    Json(payload): Json<RoundResult>,
) -> impl IntoResponse {
    match tournament_service::update_result(&pool, id, claims, &payload).await {
        Ok(()) => {
            let response: AppResponse = payload.into();
            response.into_response()
        }
        Err(e) => Into::<AppError>::into(e).into_response(),
    }
}

async fn update_player_status(
    State(pool): State<SqlitePool>,
    Path(tournament_id): Path<u32>,
    CurrentUser(claims): CurrentUser,
    Json(payload): Json<PlayerStatusPayload>,
) -> impl IntoResponse {
    match tournament_service::update_player_status(&pool, tournament_id, claims, &payload).await {
        Ok(_) => AppResponse::Success {
            payload: SuccessResponse::StatusUpdated {
                registration_id: payload.id,
                status: payload.status,
            },
        }
        .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn end_tournament(
    State(pool): State<SqlitePool>,
    Path(tournament_id): Path<u32>,
    CurrentUser(claims): CurrentUser,
) -> impl IntoResponse {
    match tournament_service::end_tournament(&pool, tournament_id, claims).await {
        Ok(timestamp) => AppResponse::Success {
            payload: SuccessResponse::TournamentEnded { timestamp },
        }
        .into_response(),
        Err(e) => Into::<AppError>::into(e).into_response(),
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(list_tournaments))
        .route("/", post(create_tournament))
        .route("/{id}", get(get_tournament))
        .route("/{id}/pair", post(generate_next_round_pairings))
        .route("/{id}/register", post(register_player))
        .route("/{id}/result", post(update_game_result))
        .route("/{id}/end", post(end_tournament))
        .route("/{id}/player-status", post(update_player_status))
        .with_state(state)
}
