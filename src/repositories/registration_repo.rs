use sqlx::prelude::FromRow;

use crate::{
    models::tournament::{PlayerResult, PlayerStatus},
    payloads::NewRegistration,
    repositories::pairing_repo::DbPairing,
};

pub async fn create_tournament_registration(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
    payload: NewRegistration,
) -> sqlx::Result<i64> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query("insert into registrations (player_id, tournament_id, floats, status, rating) values (?1, ?2, ?3, ?4, ?5)")
        .bind(payload.player_id)
        .bind(tournament_id)
        .bind(0)
        .bind(payload.status)
        .bind(payload.rating)
        .execute(&mut *tx)
        .await?;
    let registration_id = result.last_insert_rowid();
    let current_pairings: Vec<DbPairing> =
        sqlx::query_as("select * from pairings where tournament_id = ?1")
            .bind(tournament_id)
            .fetch_all(&mut *tx)
            .await?;
    if !current_pairings.is_empty() {
        let last_round = current_pairings
            .iter()
            .map(|pair| pair.round_number)
            .max()
            .unwrap();
        for round_id in 0u32..=last_round as u32 {
            let score = match payload.absent_results.get(round_id as usize) {
                Some(result) => match PlayerResult::from_str(result) {
                    PlayerResult::Win => 2,
                    PlayerResult::Draw => 1,
                    PlayerResult::Lose => 0,
                },
                None => 0,
            };
            sqlx::query("insert into pairing_gaps (player_id, tournament_id, is_bye, round_id, score) values (?1, ?2, ?3, ?4, ?5)")
                .bind(registration_id)
                .bind(tournament_id)
                .bind(false)
                .bind(round_id)
                .bind(score)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(registration_id)
}

pub async fn update_registration_status(
    pool: &sqlx::SqlitePool,
    registration_id: u32,
    status: PlayerStatus,
) -> sqlx::Result<()> {
    sqlx::query("update registrations set status = ?1 where id = ?2")
        .bind(status.to_string())
        .bind(registration_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(FromRow)]
pub struct DbRegistration {
    pub id: u32,
    pub floats: u32,
    pub status: String,
    pub player_id: u32,
    pub rating: u32,
    pub first_name: String,
    pub last_name: String,
    pub federation: Option<String>,
    pub fide_id: Option<u32>,
    pub title: String,
}

pub async fn select_registrations(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
) -> sqlx::Result<Vec<DbRegistration>> {
    let registrations: Vec<DbRegistration> = sqlx::query_as(
        "select
            r.id,
            r.floats,
            r.status,
            r.player_id,
            r.rating,
            p.first_name,
            p.last_name,
            p.federation,
            p.fide_id,
            p.title
        from registrations r
        inner join players p on r.player_id = p.id
        where r.tournament_id = ?",
    )
    .bind(tournament_id)
    .fetch_all(pool)
    .await?;
    Ok(registrations)
}

#[cfg(test)]
mod tests {
    use crate::models::tournament::PlayerStatus;

    use super::*;

    #[sqlx::test(fixtures(
        path = "../../fixtures",
        scripts("create_players", "create_user", "create_tournament")
    ))]
    async fn test_register_player(pool: sqlx::SqlitePool) {
        let payload = NewRegistration {
            player_id: 1,
            status: PlayerStatus::Active.to_string(),
            rating: 2000,
            absent_results: Vec::new(),
        };
        create_tournament_registration(&pool, 1, payload)
            .await
            .expect("failed to register player 1");
        let payload = NewRegistration {
            player_id: 2,
            status: PlayerStatus::Active.to_string(),
            rating: 2000,
            absent_results: Vec::new(),
        };
        create_tournament_registration(&pool, 1, payload)
            .await
            .expect("failed to register player 2");
    }
}
