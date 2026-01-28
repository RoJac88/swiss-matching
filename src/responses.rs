use axum::{
    Json as AxumJson,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use itertools::Itertools;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    errors::AppError,
    models::tournament::{HistoryItem, NewPairings, PlayerStanding, Tournament},
    payloads::{NewPlayer, RoundResult},
    repositories::{player_repo::DbPlayer, tournament_repo::DbTournament},
};

#[derive(Debug, Serialize)]
#[serde(tag = "status")]
#[serde(rename_all = "camelCase")]
pub enum AppResponse {
    Error { error: ErrorResponse },
    Success { payload: SuccessResponse },
}

pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
    AxumJson<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match AxumJson::<T>::from_request(req, state).await {
            Ok(json) => Ok(Json(json.0)),
            Err(rej) => match rej {
                JsonRejection::JsonDataError(_) => Err(AppError::JsonDataError),
                JsonRejection::JsonSyntaxError(e) => Err(AppError::JsonSyntaxError(e.to_string())),
                JsonRejection::MissingJsonContentType(_) => Err(AppError::MissingContentType),
                _ => Err(AppError::JsonUnknownError),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredPlayer {
    id: u32,
    player_id: u32,
    name: String,
    title: String,
    federation: Option<String>,
    rating: u32,
    fide_id: Option<usize>,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FidePlayer {
    pub fide_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub federation: Option<String>,
    pub title: Option<String>,
    pub rating: Option<u32>,
    pub rating_rapid: Option<u32>,
    pub rating_blitz: Option<u32>,
}

impl From<FidePlayer> for NewPlayer {
    fn from(value: FidePlayer) -> Self {
        Self {
            first_name: value.first_name,
            last_name: value.last_name,
            federation: value.federation,
            fide_id: Some(value.fide_id),
            title: value.title,
            rating: value.rating,
            rating_rapid: value.rating_rapid,
            rating_blitz: value.rating_blitz,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundPairing {
    board_number: u32,
    white_id: u32,
    black_id: u32,
    result: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundGap {
    player_id: u32,
    score: u32,
    is_bye: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentItem {
    id: u32,
    name: String,
    current_round: u32,
    num_rounds: u32,
    time_category: String,
    federation: String,
    user_id: u32,
    username: String,
    updated_at: u32,
    end_date: Option<u32>,
    url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing)]
    pub status_code: StatusCode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum SuccessResponse {
    UserCreated {
        id: i64,
    },
    TournamentEnded {
        timestamp: i64,
    },
    PlayerCreated {
        id: i64,
    },
    PlayerExists {
        id: u32,
        fide_id: i64,
    },
    PlayerUpdated {
        player: DbPlayer,
    },
    TournamentCreated {
        id: i64,
    },
    PlayerList {
        players: Vec<DbPlayer>,
    },
    PlayerRegistered {
        id: i64,
    },
    PairingGenerated {
        round: u32,
        pairings: Vec<(u32, u32)>,
        not_paired: Vec<u32>,
        byes: Vec<u32>,
    },
    TournamentData {
        id: u32,
        name: String,
        current_round: u32,
        num_rounds: u32,
        time_category: String,
        start_date: usize,
        federation: String,
        players: Vec<RegisteredPlayer>,
        pairings: Vec<Vec<RoundPairing>>,
        gaps: Vec<Vec<RoundGap>>,
        standings: Vec<Vec<PlayerStanding>>,
        user_id: u32,
        username: String,
        updated_at: u32,
        end_date: Option<u32>,
        url: Option<String>,
    },
    TournamentList {
        tournaments: Vec<TournamentItem>,
    },
    ResultUpdated {
        board_id: u32,
        game_result: String,
    },
    StatusUpdated {
        registration_id: u32,
        status: String,
    },
    FidePlayer {
        player: FidePlayer,
    },
    LoginSuccess {
        token: String,
        role: String,
    },
}

impl From<NewPairings> for AppResponse {
    fn from(value: NewPairings) -> Self {
        let pairings = value
            .pairings
            .iter()
            .map(|pair| (pair.white_id, pair.black_id))
            .collect();
        let not_paired = value
            .gaps
            .iter()
            .filter(|g| !g.is_bye)
            .map(|gap| gap.player_id)
            .collect();
        let byes = value
            .gaps
            .iter()
            .filter(|g| g.is_bye)
            .map(|gap| gap.player_id)
            .collect();
        Self::Success {
            payload: SuccessResponse::PairingGenerated {
                round: value.round,
                pairings,
                not_paired,
                byes,
            },
        }
    }
}

impl From<Tournament> for AppResponse {
    fn from(value: Tournament) -> Self {
        let mut pairings: Vec<Vec<RoundPairing>> = value
            .pairings
            .iter()
            .map(|round| {
                round
                    .iter()
                    .enumerate()
                    .map(|(board_number, (white_id, black_id))| RoundPairing {
                        board_number: board_number as u32,
                        white_id: *white_id as u32,
                        black_id: *black_id as u32,
                        result: None,
                    })
                    .collect()
            })
            .collect();
        for (round_number, round) in value.results.iter().enumerate() {
            for (board, game_result) in round.iter().enumerate() {
                pairings[round_number][board].result = Some(game_result.to_string());
            }
        }
        let mut gaps: Vec<Vec<RoundGap>> = (0..value.current_round()).map(|_| Vec::new()).collect();
        for player in value.players.values() {
            for (round, item) in player.history.iter().enumerate() {
                match item {
                    HistoryItem::NotPaired { score } => {
                        gaps[round].push(RoundGap {
                            player_id: player.id,
                            score: *score,
                            is_bye: false,
                        });
                    }
                    HistoryItem::Bye => {
                        gaps[round].push(RoundGap {
                            player_id: player.id,
                            score: 2,
                            is_bye: true,
                        });
                    }
                    _ => {}
                }
            }
        }
        Self::Success {
            payload: SuccessResponse::TournamentData {
                id: value.id,
                name: value.name.clone(),
                current_round: value.current_round() as u32,
                num_rounds: value.num_rounds as u32,
                time_category: value.time_category.clone(),
                start_date: value.start_date,
                federation: value.federation.clone(),
                end_date: value.end_date,
                players: value
                    .players
                    .values()
                    .map(|p| RegisteredPlayer {
                        id: p.id,
                        player_id: p.db_id,
                        name: p.name.clone(),
                        title: p.title.to_string(),
                        federation: p.federation.clone(),
                        fide_id: p.fide_id,
                        rating: p.rating,
                        status: p.status.to_string(),
                    })
                    .sorted_unstable_by(|a, b| a.id.cmp(&b.id))
                    .collect(),
                pairings,
                standings: value.standings(),
                url: value.url,
                gaps,
                user_id: value.user_id,
                username: value.username,
                updated_at: value.updated_at,
            },
        }
    }
}

impl From<Vec<DbTournament>> for AppResponse {
    fn from(value: Vec<DbTournament>) -> Self {
        Self::Success {
            payload: SuccessResponse::TournamentList {
                tournaments: value
                    .into_iter()
                    .map(|t| TournamentItem {
                        id: t.id,
                        name: t.name,
                        num_rounds: t.num_rounds,
                        current_round: t.current_round,
                        time_category: t.time_category,
                        end_date: t.end_date,
                        federation: t.federation,
                        url: t.url,
                        user_id: t.user_id,
                        username: t.username,
                        updated_at: t.updated_at,
                    })
                    .collect(),
            },
        }
    }
}

impl From<RoundResult> for AppResponse {
    fn from(value: RoundResult) -> Self {
        Self::Success {
            payload: SuccessResponse::ResultUpdated {
                board_id: value.board_id,
                game_result: value.result,
            },
        }
    }
}

impl From<FidePlayer> for AppResponse {
    fn from(value: FidePlayer) -> Self {
        Self::Success {
            payload: SuccessResponse::FidePlayer { player: value },
        }
    }
}

impl IntoResponse for AppResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppResponse::Error { error: e } => (e.status_code, AxumJson(e)).into_response(),
            AppResponse::Success { payload: _ } => (StatusCode::OK, AxumJson(self)).into_response(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match &self {
            AppError::EmptyPairingsGenerated => StatusCode::BAD_REQUEST,
            AppError::InvalidPlayerStatus(_) => StatusCode::BAD_REQUEST,
            AppError::DuplicatePlayerResult(_) => StatusCode::BAD_REQUEST,
            AppError::RoundNotDone => StatusCode::BAD_REQUEST,
            AppError::InvalidPlayerId(_) => StatusCode::NOT_FOUND,
            AppError::InvalidPlayerScore(_) => StatusCode::BAD_REQUEST,
            AppError::InvalidTimeCategory(_) => StatusCode::BAD_REQUEST,
            AppError::InvalidNumberOfRounds(_) => StatusCode::BAD_REQUEST,
            AppError::RoundNotFound(_) => StatusCode::NOT_FOUND,
            AppError::GameNotFound { round: _, game: _ } => StatusCode::NOT_FOUND,
            AppError::PlayerNotFound(_) => StatusCode::NOT_FOUND,
            AppError::InsertGameHistorySkipsRound => StatusCode::BAD_REQUEST,
            AppError::TournamentEnded => StatusCode::BAD_REQUEST,
            AppError::TournamentNotStarted => StatusCode::BAD_REQUEST,
            AppError::InvalidRound(_) => StatusCode::NOT_FOUND,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InsufficientPlayers => StatusCode::BAD_REQUEST,
            AppError::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::FideScrapeFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::MissingContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            AppError::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
            AppError::JsonDataError => StatusCode::BAD_REQUEST,
            AppError::JsonUnknownError => StatusCode::BAD_REQUEST,
            AppError::LoginFailed(_) => StatusCode::UNAUTHORIZED,
            AppError::UsernameTaken(_) => StatusCode::BAD_REQUEST,
            AppError::TournamentNotFound => StatusCode::NOT_FOUND,
            AppError::InsufficientPermissions => StatusCode::UNAUTHORIZED,
            AppError::CannotEndTournament => StatusCode::BAD_REQUEST,
            AppError::TokenInvalid => StatusCode::UNAUTHORIZED,
            AppError::InvalidAuthHeader => StatusCode::UNAUTHORIZED,
        };
        AxumJson(AppResponse::Error {
            error: ErrorResponse {
                code: self.code(),
                message: format!("{}", self),
                status_code,
            },
        })
        .into_response()
    }
}
