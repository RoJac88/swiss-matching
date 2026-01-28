use sqlx::prelude::FromRow;

use crate::{
    models::tournament::GameResult, repositories::tournament_repo::mark_tournament_updated,
};

#[derive(FromRow)]
pub struct DbPairing {
    pub id: u32,
    pub tournament_id: u32,
    pub round_number: u32,
    pub board_number: u32,
    pub white_id: u32,
    pub black_id: u32,
    pub result: Option<String>,
    pub pgn: Option<String>,
}

pub struct NewDbPairing {
    pub tournament_id: u32,
    pub round_number: u32,
    pub board_number: u32,
    pub white_id: u32,
    pub black_id: u32,
}

pub async fn select_pairings(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
) -> sqlx::Result<Vec<DbPairing>> {
    sqlx::query_as("select * from pairings where tournament_id = ?")
        .bind(tournament_id)
        .fetch_all(pool)
        .await
}
#[derive(FromRow)]
pub struct DbPairingGap {
    pub id: u32,
    pub player_id: u32,
    pub tournament_id: u32,
    pub round_id: u32,
    pub score: u32,
    pub is_bye: bool,
}

#[derive(Debug)]
pub struct NewDbPairingGap {
    pub player_id: u32,
    pub tournament_id: u32,
    pub round_id: u32,
    pub score: u32,
    pub is_bye: bool,
}

pub async fn select_pairing_gaps(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
) -> sqlx::Result<Vec<DbPairingGap>> {
    sqlx::query_as("select * from pairing_gaps where tournament_id = ?")
        .bind(tournament_id)
        .fetch_all(pool)
        .await
}

pub async fn update_game_result(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
    round_id: u32,
    board_id: u32,
    result: GameResult,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("update pairings set result = ?1 where tournament_id = ?2 and round_number = ?3 and board_number = ?4")
        .bind(result.to_string())
        .bind(tournament_id)
        .bind(round_id)
        .bind(board_id)
        .execute(&mut *tx)
        .await?;
    mark_tournament_updated(tournament_id, &mut tx).await?;
    tx.commit().await?;
    Ok(())
}
