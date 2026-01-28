use chrono::Utc;
use serde::Serialize;
use sqlx::prelude::FromRow;

use crate::payloads::NewPlayer;

pub async fn create_player(pool: &sqlx::SqlitePool, player: NewPlayer) -> sqlx::Result<i64> {
    let now = Utc::now();
    let result = sqlx::query(
        "insert into players
            (first_name, last_name, federation, fide_id, title, rating, rating_rapid, rating_blitz, updated_at)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
    )
    .bind(player.first_name)
    .bind(player.last_name)
    .bind(player.federation)
    .bind(player.fide_id)
    .bind(player.title)
    .bind(player.rating)
    .bind(player.rating_rapid)
    .bind(player.rating_blitz)
    .bind(now.timestamp())
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn get_player_by_fide_id(
    pool: &sqlx::SqlitePool,
    fide_id: i64,
) -> sqlx::Result<Option<DbPlayer>> {
    sqlx::query_as("select * from players where fide_id = ?1")
        .bind(&fide_id)
        .fetch_optional(pool)
        .await
}

pub async fn update_fide_player(pool: &sqlx::SqlitePool, player: NewPlayer) -> sqlx::Result<i64> {
    let now = Utc::now();
    sqlx::query(
        "update players set
            first_name = ?1,
            last_name = ?2,
            federation = ?3,
            title = ?4,
            rating = ?5,
            rating_rapid = ?6,
            rating_blitz = ?7,
            updated_at = ?8
        where fide_id = ?9",
    )
    .bind(player.first_name)
    .bind(player.last_name)
    .bind(player.federation)
    .bind(player.title)
    .bind(player.rating)
    .bind(player.rating_rapid)
    .bind(player.rating_blitz)
    .bind(now.timestamp())
    .bind(player.fide_id)
    .execute(pool)
    .await?;
    Ok(now.timestamp())
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DbPlayer {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub updated_at: u32,
    pub federation: Option<String>,
    pub fide_id: Option<i64>,
    pub title: Option<String>,
    pub rating: Option<u32>,
    pub rating_rapid: Option<u32>,
    pub rating_blitz: Option<u32>,
}

pub async fn list_players(pool: &sqlx::SqlitePool) -> sqlx::Result<Vec<DbPlayer>> {
    sqlx::query_as("select * from players")
        .fetch_all(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_create_player(pool: sqlx::SqlitePool) {
        let new_player = NewPlayer {
            first_name: "Rodrigo".to_string(),
            last_name: "Jacob".to_string(),
            federation: None,
            fide_id: None,
            title: None,
            rating: Some(2099),
            rating_rapid: None,
            rating_blitz: None,
        };
        let id = create_player(&pool, new_player)
            .await
            .expect("Player inserted");
        assert!(id >= 0);
    }
    #[sqlx::test(fixtures(path = "../../fixtures", scripts("create_players")))]
    async fn test_list_players(pool: sqlx::SqlitePool) {
        let players = list_players(&pool).await.expect("failed to list players");
        assert_eq!(players.len(), 101);
        assert_eq!(players[0].first_name, String::from("Magnus"));
    }
}
