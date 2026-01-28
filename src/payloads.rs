use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPlayer {
    pub first_name: String,
    pub last_name: String,
    pub federation: Option<String>,
    pub fide_id: Option<i64>,
    pub title: Option<String>,
    pub rating: Option<u32>,
    pub rating_rapid: Option<u32>,
    pub rating_blitz: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTournament {
    pub name: String,
    pub rounds: u32,
    pub time_category: String,
    pub start_date: u32,
    pub federation: String,
    pub url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRegistration {
    pub player_id: i64,
    pub rating: u32,
    pub status: String,
    pub absent_results: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextPairings {
    pub first_color: Option<String>,
    pub inactive_scores: Vec<(u32, String)>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub round_id: u32,
    pub board_id: u32,
    pub result: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatusPayload {
    pub id: u32,
    pub status: String,
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct NewUser {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}
