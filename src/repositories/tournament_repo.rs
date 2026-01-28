use chrono::Utc;
use sqlx::{Sqlite, Transaction, prelude::FromRow};

use crate::{
    auth::jwt::Claims, errors::AppError, models::tournament::NewPairings, payloads::NewTournament,
};

pub async fn create_tournament(
    pool: &sqlx::SqlitePool,
    user_id: u32,
    payload: NewTournament,
) -> sqlx::Result<i64> {
    let result =
        sqlx::query("insert into tournaments (created_by, name, num_rounds, time_category, start_date, federation, url, current_round) values (?, ?, ?, ?, ?, ?, ?, 0)")
            .bind(user_id)
            .bind(&payload.name)
            .bind(&payload.rounds)
            .bind(&payload.time_category)
            .bind(&payload.start_date)
            .bind(&payload.federation)
            .bind(&payload.url)
            .execute(pool)
            .await?;
    Ok(result.last_insert_rowid())
}

#[derive(Debug, FromRow)]
struct TournamentOwnerAndEndDate {
    created_by: u32,
    end_date: Option<u32>,
}

// Cannot edit tournaments that have already ended
// Users can only edit tournaments they created
// Admin can edit any tournament that has not ended
pub async fn check_user_tournament_permissions(
    pool: &sqlx::SqlitePool,
    tournament_id: u32,
    claims: Claims,
) -> Result<bool, AppError> {
    let tourn: Option<TournamentOwnerAndEndDate> =
        match sqlx::query_as("select created_by, end_date from tournaments where id = ?")
            .bind(tournament_id)
            .fetch_optional(pool)
            .await
        {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("check_user_tournament_permissions: {:?}", e);
                return Err(AppError::Unknown);
            }
        };
    if let Some(t) = tourn {
        if t.end_date.is_some() {
            return Ok(false);
        }
        if t.created_by == claims.sub || claims.role == "admin" {
            return Ok(true);
        }
        return Ok(false);
    } else {
        return Err(AppError::TournamentNotFound);
    }
}

pub async fn mark_tournament_updated(
    tournament_id: u32,
    tx: &mut Transaction<'_, Sqlite>,
) -> sqlx::Result<()> {
    let now = Utc::now();
    let _ = sqlx::query("update tournaments set updated_at = ? where id = ?")
        .bind(now.timestamp())
        .bind(tournament_id)
        .execute(tx.as_mut())
        .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
pub struct DbTournament {
    pub id: u32,
    pub name: String,
    pub current_round: u32,
    pub num_rounds: u32,
    pub time_category: String,
    pub start_date: u32,
    pub federation: String,
    pub username: String,
    pub user_id: u32,
    pub updated_at: u32,
    pub end_date: Option<u32>,
    pub url: Option<String>,
}

pub async fn list_tournaments(pool: &sqlx::SqlitePool) -> sqlx::Result<Vec<DbTournament>> {
    sqlx::query_as("select
            t.id, t.name, t.current_round, t.num_rounds, t.time_category, t.start_date, t.federation, t.end_date, t.url, t.updated_at, u.id as user_id, u.username as username
            from tournaments t
            inner join users u on t.created_by = u.id
            order by t.updated_at desc"
        )
        .fetch_all(pool)
        .await
}

pub async fn get_tournament(pool: &sqlx::SqlitePool, id: u32) -> sqlx::Result<DbTournament> {
    sqlx::query_as("select
        t.id, t.name, t.current_round, t.num_rounds, t.time_category, t.start_date, t.federation, t.end_date, t.url, t.updated_at, t.url, u.id as user_id, u.username as username
        from tournaments t
        inner join users u on u.id = t.created_by
        where t.id = ?1")
        .bind(&id)
        .fetch_one(pool)
        .await
}

impl NewPairings {
    pub async fn commit(&self, pool: &sqlx::Pool<sqlx::Sqlite>) -> sqlx::Result<()> {
        let mut tx = pool.begin().await?;
        for pairing in self.pairings.iter() {
            sqlx::query("insert into pairings (tournament_id, round_number, board_number, white_id, black_id) values (?1, ?2, ?3, ?4, ?5)")
                .bind(pairing.tournament_id)
                .bind(pairing.round_number)
                .bind(pairing.board_number)
                .bind(pairing.white_id)
                .bind(pairing.black_id)
                .execute(&mut *tx)
                .await?;
        }
        for gap in self.gaps.iter() {
            sqlx::query("insert into pairing_gaps (tournament_id, player_id, round_id, score, is_bye) values (?1, ?2, ?3, ?4, ?5)")
                .bind(gap.tournament_id)
                .bind(gap.player_id)
                .bind(gap.round_id)
                .bind(gap.score)
                .bind(gap.is_bye)
                .execute(&mut *tx)
                .await?;
        }
        for id in self.floats.iter() {
            sqlx::query("update registrations set floats = floats + 1 where id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("update tournaments set current_round = current_round + 1 where id = ?1")
            .bind(self.pairings[0].tournament_id)
            .execute(&mut *tx)
            .await?;
        mark_tournament_updated(self.pairings[0].tournament_id, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

pub async fn end_tournament(pool: &sqlx::SqlitePool, tournament_id: u32) -> sqlx::Result<i64> {
    let now = Utc::now().timestamp();
    let _ = sqlx::query("update tournaments set end_date = ?, updated_at = ? where id = ?")
        .bind(now)
        .bind(now)
        .bind(tournament_id)
        .execute(pool)
        .await?;
    Ok(now)
}

#[cfg(test)]
mod tests {
    use crate::{
        models::tournament::{Color, Tournament},
        services::tournament_service,
    };

    use super::*;

    #[sqlx::test(fixtures(path = "../../fixtures", scripts("create_user",)))]
    async fn test_create_standard_tournament(pool: sqlx::SqlitePool) {
        let new_tournament = NewTournament {
            name: "Test Tournament".to_string(),
            rounds: 9,
            time_category: "standard".to_string(),
            start_date: 0,
            federation: "FID".to_string(),
            url: None,
        };
        let id = create_tournament(&pool, 1, new_tournament)
            .await
            .expect("Failed to create tournament");
        assert_eq!(id, 1);
    }
    #[sqlx::test(fixtures(
        path = "../../fixtures",
        scripts(
            "create_players",
            "create_user",
            "create_tournament",
            "register_players"
        )
    ))]
    async fn test_first_pairing(pool: sqlx::SqlitePool) {
        let tournament = tournament_service::read_tournament(&pool, 1)
            .await
            .expect("failed to read_tournament");
        let tournament: Tournament = tournament.into();
        let new_pairings = tournament
            .generate_first_round_pairings(tournament_service::InactiveScores::new(), Color::White)
            .expect("failed to generate first round pairings");
        for pair in new_pairings.pairings.iter() {
            println!(
                "[{}]: {} - {}",
                pair.board_number, pair.white_id, pair.black_id
            )
        }
        assert_eq!(new_pairings.pairings.len(), 25)
    }
}
