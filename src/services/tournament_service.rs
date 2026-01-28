use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use itertools::Itertools;
use rustworkx_core::{
    max_weight_matching::max_weight_matching,
    petgraph::{graph, visit::EdgeRef},
};

use crate::{
    auth::jwt::Claims,
    errors::AppError,
    models::tournament::{
        Color, GameResult, HistoryItem, NewPairings, Player, PlayerResult, PlayerStanding,
        PlayerStatus, Title, Tournament, TournamentDbData,
    },
    payloads::{NewRegistration, NewTournament, NextPairings, PlayerStatusPayload, RoundResult},
    repositories::{
        pairing_repo::{
            NewDbPairing, NewDbPairingGap, select_pairing_gaps, select_pairings, update_game_result,
        },
        registration_repo::{self, select_registrations},
        tournament_repo::{self, DbTournament, check_user_tournament_permissions, get_tournament},
    },
    responses::AppResponse,
};

enum TimeCategory {
    Blitz,
    Rapid,
    Standard,
}

impl TryFrom<&String> for TimeCategory {
    type Error = AppError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.trim().to_lowercase().as_str() {
            "blitz" => Ok(Self::Blitz),
            "rapid" => Ok(Self::Rapid),
            "standard" => Ok(Self::Standard),
            _ => Err(AppError::InvalidTimeCategory(value.to_string())),
        }
    }
}

pub async fn create_tournament(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    user_id: u32,
    payload: NewTournament,
) -> Result<i64, AppError> {
    TimeCategory::try_from(&payload.time_category)?;
    if payload.rounds < 2 || payload.rounds > 30 {
        return Err(AppError::InvalidNumberOfRounds(payload.rounds));
    }
    let id = tournament_repo::create_tournament(pool, user_id, payload).await?;
    Ok(id)
}

pub async fn register_player(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    tournament_id: u32,
    claims: Claims,
    payload: NewRegistration,
) -> Result<i64, AppError> {
    let has_permission = check_user_tournament_permissions(pool, tournament_id, claims).await?;
    if !has_permission {
        return Err(AppError::InsufficientPermissions);
    }
    registration_repo::create_tournament_registration(pool, tournament_id, payload)
        .await
        .map_err(|e| Into::<AppError>::into(e))
}

impl Player {
    fn tournament_score(&self) -> u32 {
        self.history.iter().fold(0, |acc, item| match item {
            HistoryItem::NotPaired { score } => acc + *score,
            HistoryItem::Bye => acc + 2,
            HistoryItem::Game {
                opponent_id: _,
                color,
                result,
            } => match (color, result) {
                (Color::White, GameResult::WhiteWins) => acc + 2,
                (Color::White, GameResult::Draw) => acc + 1,
                (Color::Black, GameResult::Draw) => acc + 1,
                (Color::Black, GameResult::BlackWins) => acc + 2,
                _ => acc,
            },
        })
    }
    fn byes(&self) -> usize {
        self.history
            .iter()
            .filter(|h| **h == HistoryItem::Bye)
            .count()
    }
}

impl From<TournamentDbData> for Tournament {
    fn from(value: TournamentDbData) -> Self {
        let mut players: HashMap<u32, Player> = value
            .players
            .into_iter()
            .map(|p| {
                (
                    p.id,
                    Player {
                        id: p.id,
                        db_id: p.player_id,
                        name: format!("{}, {}", p.last_name, p.first_name),
                        rating: p.rating,
                        title: Title::from_str(p.title),
                        history: (0..value.tournament.current_round)
                            .map(|_| HistoryItem::NotPaired { score: 0 })
                            .collect(),
                        floats: p.floats as usize,
                        fide_id: p.fide_id.map(|id| id as usize),
                        federation: p.federation,
                        status: PlayerStatus::from_str(p.status),
                    },
                )
            })
            .collect();
        let mut results: Vec<Vec<(usize, GameResult)>> = (0..value.tournament.current_round)
            .map(|_| Vec::new())
            .collect();
        let mut byes: Vec<Vec<u32>> = (0..value.tournament.current_round)
            .map(|_| Vec::new())
            .collect();
        for gap in value.pairing_gaps.iter() {
            let history_item = match gap.is_bye {
                true => {
                    byes[gap.round_id as usize].push(gap.player_id);
                    HistoryItem::Bye
                }
                false => HistoryItem::NotPaired { score: gap.score },
            };
            let player = players.get_mut(&gap.player_id).unwrap();
            player.history[gap.round_id as usize] = history_item;
        }
        for pairing in value.pairings.iter() {
            let result = match pairing.result.as_ref() {
                Some(s) => GameResult::from_str(s),
                None => GameResult::Ongoing,
            };
            results[pairing.round_number as usize].push((pairing.board_number as usize, result));
            let history_item_white = HistoryItem::Game {
                opponent_id: pairing.black_id,
                color: Color::White,
                result,
            };
            let white = players.get_mut(&pairing.white_id).unwrap();
            white.history[pairing.round_number as usize] = history_item_white;
            let history_item_black = HistoryItem::Game {
                opponent_id: pairing.white_id,
                color: Color::Black,
                result,
            };
            let black = players.get_mut(&pairing.black_id).unwrap();
            black.history[pairing.round_number as usize] = history_item_black;
        }
        let mut round_pairings: Vec<Vec<(usize, usize, usize)>> =
            (0..value.tournament.current_round)
                .map(|_| Vec::new())
                .collect();
        for pairing in value.pairings {
            round_pairings[pairing.round_number as usize].push((
                pairing.board_number as usize,
                pairing.white_id as usize,
                pairing.black_id as usize,
            ));
        }
        for round_pairing in round_pairings.iter_mut() {
            round_pairing.sort_by(|a, b| a.0.cmp(&b.0));
        }
        for result in results.iter_mut() {
            result.sort_by(|a, b| a.0.cmp(&b.0));
        }
        Self {
            id: value.tournament.id,
            name: value.tournament.name,
            num_rounds: value.tournament.num_rounds as usize,
            time_category: value.tournament.time_category,
            players,
            pairings: round_pairings
                .into_iter()
                .map(|round| {
                    round
                        .into_iter()
                        .map(|(_, white, black)| (white, black))
                        .collect()
                })
                .collect(),
            byes,
            results: results
                .into_iter()
                .map(|round| round.into_iter().map(|(_, res)| res).collect())
                .collect(),
            federation: value.tournament.federation,
            start_date: value.tournament.start_date as usize,
            end_date: value.tournament.end_date,
            url: value.tournament.url,
            user_id: value.tournament.user_id,
            username: value.tournament.username,
            updated_at: value.tournament.updated_at,
        }
    }
}

pub async fn read_tournament(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    id: u32,
) -> Result<TournamentDbData, AppError> {
    let tournament = match get_tournament(pool, id).await {
        Ok(t) => t,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::TournamentNotFound),
        Err(e) => return Err(AppError::Database(e)),
    };
    let registrations = select_registrations(pool, id).await?;
    let pairings = select_pairings(pool, id).await?;
    let gaps = select_pairing_gaps(pool, id).await?;
    let tournament_data = TournamentDbData {
        tournament,
        players: registrations,
        pairings,
        pairing_gaps: gaps,
    };
    Ok(tournament_data)
}

pub async fn list_tournaments(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<Vec<DbTournament>, AppError> {
    tournament_repo::list_tournaments(pool)
        .await
        .map_err(|e| Into::<AppError>::into(e))
}

fn edge_weight(
    p1: &Player,
    p2: &Player,
    group_ranks: (usize, usize),
    group_len: (usize, usize),
    min_score: u32,
) -> isize {
    let p1_colors = p1.color_history();
    let p2_colors = p2.color_history();
    // Players cannot play 3 times with the same color
    if let (Some(p1_last_2_colors), Some(p2_last_2_colors)) =
        (p1_colors.last_chunk::<2>(), p2_colors.last_chunk::<2>())
    {
        if p1_last_2_colors[0] == p1_last_2_colors[1] && p1_last_2_colors == p2_last_2_colors {
            tracing::debug!(
                "\n----- Paring calculation for {} vs {}-----\n",
                p1.name,
                p2.name
            );
            tracing::debug!(
                "Cannot repeat colors three times, returning min value: {}",
                isize::MIN
            );
            return isize::MIN;
        }
    }
    let mut weight: isize = 5_000;
    let scores = (p1.tournament_score(), p2.tournament_score());
    let score_diff = scores.0.abs_diff(scores.1);
    // Score similarity (main criterion)
    let score_penalty = match score_diff {
        0 => 0,    // same score – best
        1 => 80,   // natural float (win vs draw) – very acceptable
        2 => 570,  // full point gap (win vs loss) – allowed when needed
        3 => 1350, // 1.5 traditional points – strongly discourage
        4 => 2250, // 2.0 traditional points
        _ => 2250 + (score_diff as isize) * 200,
    };
    weight -= score_penalty;

    // Small bonus for higher combined score (tends to pair leaders together)
    weight += ((scores.0 + scores.1) as isize) * 5;

    // Color balance
    let color_penalty = if let (Some(p1_last), Some(p2_last)) = (p1_colors.last(), p2_colors.last())
    {
        if p1_last == p2_last { 10 } else { 0 }
    } else {
        0
    };
    weight -= color_penalty;

    // Within same score group: prefer top-half vs bottom-half
    let half_pair_deviation_penalty = if scores.0 == scores.1 {
        let mid = group_len.0 / 2;
        let dist = group_ranks.0.abs_diff(group_ranks.1) as isize;
        // Reward pairs that are far apart in ranking
        let ideal_dist = mid as isize;
        let deviation = (dist - ideal_dist).abs();
        deviation * 5 // penalize pairs that are too close or too far
    } else {
        0
    };
    weight -= half_pair_deviation_penalty;

    // Penalize repeated floats
    let repeated_float_penalty = (p1.floats as isize + p2.floats as isize) * 20;
    weight -= repeated_float_penalty;

    // Isolation bonus
    let isolation_bonus = if scores.0 != min_score && scores.1 != min_score {
        200 / (group_len.0.max(group_len.1) as isize)
    } else {
        0
    };
    weight += isolation_bonus;

    // Floating behavior: discourage pairing floated-up player with top of higher group
    let float_rank_penalty = if scores.0 != scores.1 {
        // identify who is lower score
        let (_low_rank, high_rank, high_group_len) = if scores.0 < scores.1 {
            (group_ranks.0, group_ranks.1, group_len.1)
        } else {
            (group_ranks.1, group_ranks.0, group_len.0)
        };

        // rank 0 = top of group → biggest penalty
        // bottom of group → minimal penalty
        let normalized = high_rank as isize;
        let max_rank = (high_group_len.saturating_sub(1)) as isize;

        // invert so top gets punished
        (max_rank - normalized) * 10
    } else {
        0
    };

    weight -= float_rank_penalty;

    // tracing::debug!(
    //     "\n----- Paring calculation for {} vs {}-----\n",
    //     p1.name,
    //     p2.name
    // );
    // tracing::debug!("score penalty: {} (diff was {})", score_penalty, score_diff);
    // tracing::debug!("color penalty: {}", color_penalty);
    // tracing::debug!("repeated float penalty: {}", repeated_float_penalty);
    // tracing::debug!("isolation bonus: {}", isolation_bonus);
    // tracing::debug!(
    //     "half pair deviation penalty: {}",
    //     half_pair_deviation_penalty
    // );
    // tracing::debug!("weight: {}", weight);
    weight
}

#[derive(Debug)]
pub struct InactiveScores(HashMap<u32, PlayerResult>);

impl InactiveScores {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl Deref for InactiveScores {
    type Target = HashMap<u32, PlayerResult>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for InactiveScores {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<Vec<(u32, String)>> for InactiveScores {
    type Error = AppError;

    fn try_from(value: Vec<(u32, String)>) -> Result<Self, Self::Error> {
        let mut inner = HashMap::new();
        for (player_id, result_str) in value.into_iter() {
            let result = match result_str.as_str() {
                "win" => PlayerResult::Win,
                "draw" => PlayerResult::Draw,
                "loss" => PlayerResult::Lose,
                _ => return Err(Self::Error::DuplicatePlayerResult(player_id)),
            };
            let old = inner.insert(player_id, result);
            if old.is_some() {
                return Err(Self::Error::DuplicatePlayerResult(player_id));
            }
        }
        Ok(Self(inner))
    }
}

impl Tournament {
    fn player_tpn(&self, player_id: u32) -> usize {
        self.players
            .values()
            .sorted_by(|a, b| b.rating.cmp(&a.rating).then_with(|| a.title.cmp(&b.title)))
            .map(|player| player.id)
            .position(|id| id == player_id)
            .unwrap()
    }
    fn group_players_by_score(&self) -> HashMap<u32, Vec<&Player>> {
        let mut groups: HashMap<u32, Vec<&Player>> = HashMap::new();
        for player in self.players.values() {
            groups
                .entry(player.tournament_score())
                .and_modify(|g| g.push(player))
                .or_insert(vec![player]);
        }
        for group in groups.values_mut() {
            group.sort_by(|a, b| self.player_tpn(a.id).cmp(&self.player_tpn(b.id)));
        }
        groups
    }
    fn prepare_pairings(&self) -> Result<(Vec<(usize, usize)>, Vec<u32>, Vec<u32>), AppError> {
        let active_players_count = self
            .players
            .values()
            .filter(|p| p.status == PlayerStatus::Active)
            .count();
        let byes = if active_players_count % 2 != 0 {
            let bottom = self
                .players
                .values()
                .filter(|p| p.status == PlayerStatus::Active)
                .sorted_unstable_by(|a, b| {
                    b.byes()
                        .cmp(&a.byes())
                        .then_with(|| b.tournament_score().cmp(&a.tournament_score()))
                        .then_with(|| self.player_tpn(a.id).cmp(&self.player_tpn(b.id)))
                })
                .last()
                .unwrap();
            vec![bottom.id]
        } else {
            Vec::new()
        };
        if self.pairings.len() == self.num_rounds {
            return Err(AppError::TournamentEnded);
        }
        let groups = self.group_players_by_score();
        let mut edges = Vec::new();
        for (p1, p2) in self.players.keys().tuple_combinations() {
            if self.players[p1].status == PlayerStatus::Inactive
                || self.players[p2].status == PlayerStatus::Inactive
                || byes.contains(p1)
                || byes.contains(p2)
            {
                continue;
            }
            // skip players that have already played
            if self.players[p1].has_played(*p2) || self.players[p2].has_played(*p1) {
                continue;
            }
            edges.push((*p1, *p2));
        }
        let g = graph::UnGraph::<u32, u32>::from_edges(edges);
        let pairings = max_weight_matching(
            &g,
            true,
            |edge| {
                let p1_id = edge.source().index() as u32;
                let p2_id = edge.target().index() as u32;
                let p1 = &self.players[&p1_id];
                let p2 = &self.players[&p2_id];
                let min_score = groups.keys().min();
                let ranks = (
                    groups
                        .get(&p1.tournament_score())
                        .unwrap()
                        .iter()
                        .position(|p| p.id == edge.source().index() as u32)
                        .unwrap(),
                    groups
                        .get(&p2.tournament_score())
                        .unwrap()
                        .iter()
                        .position(|p| p.id == edge.target().index() as u32)
                        .unwrap(),
                );
                let weight = edge_weight(
                    p1,
                    p2,
                    ranks,
                    (
                        groups.get(&p1.tournament_score()).unwrap().len(),
                        groups.get(&p2.tournament_score()).unwrap().len(),
                    ),
                    *min_score.unwrap(),
                );
                i128::try_from(weight)
            },
            true,
        )
        .map_err(|e| {
            tracing::error!("prepare_pairings: {:?}", e);
            AppError::Unknown
        })?;
        let mut pairings: Vec<(usize, usize)> = pairings.into_iter().collect();
        pairings.sort_by(|a, b| {
            let w1 = &self.players[&(a.0 as u32)];
            let b1 = &self.players[&(a.1 as u32)];
            let w2 = &self.players[&(b.0 as u32)];
            let b2 = &self.players[&(b.1 as u32)];
            (std::cmp::max(w2.tournament_score(), b2.tournament_score()))
                .cmp(&(std::cmp::max(w1.tournament_score(), b1.tournament_score())))
                .then_with(|| {
                    std::cmp::min(w2.tournament_score(), b2.tournament_score())
                        .cmp(&(std::cmp::min(w1.tournament_score(), b1.tournament_score())))
                })
                .then_with(|| {
                    std::cmp::min(self.player_tpn(w1.id), self.player_tpn(b1.id)).cmp(
                        &std::cmp::min(self.player_tpn(w2.id), self.player_tpn(b2.id)),
                    )
                })
        });
        // Check for floats
        let mut floats = Vec::new();
        for (w, b) in pairings.iter() {
            let score_w = self.players[&(*w as u32)].tournament_score();
            let score_b = self.players[&(*b as u32)].tournament_score();
            if score_w > score_b {
                floats.push(*b as u32);
            }
            if score_b > score_w {
                floats.push(*w as u32);
            }
        }
        let byes = byes.into_iter().collect_vec();
        Ok((pairings, byes, floats))
    }
    fn process_pairings(
        &self,
        pairings: Vec<(usize, usize)>,
        byes: Vec<u32>,
        inactive_scores: InactiveScores,
    ) -> (Vec<NewDbPairing>, Vec<NewDbPairingGap>) {
        let db_pairings: Vec<NewDbPairing> = pairings
            .into_iter()
            .enumerate()
            .map(|(board, (white, black))| NewDbPairing {
                tournament_id: self.id,
                round_number: self.pairings.len() as u32,
                board_number: board as u32,
                white_id: white as u32,
                black_id: black as u32,
            })
            .collect();
        let mut db_gaps: Vec<NewDbPairingGap> = byes
            .iter()
            .map(|id| NewDbPairingGap {
                player_id: *id,
                tournament_id: self.id,
                round_id: self.pairings.len() as u32,
                score: 2,
                is_bye: true,
            })
            .collect();
        db_gaps.extend(
            self.players
                .values()
                .filter(|p| p.status == PlayerStatus::Inactive)
                .map(|player| match inactive_scores.get(&player.id) {
                    Some(result) => NewDbPairingGap {
                        player_id: player.id,
                        tournament_id: self.id,
                        round_id: self.pairings.len() as u32,
                        score: match result {
                            PlayerResult::Win => 2,
                            PlayerResult::Lose => 0,
                            PlayerResult::Draw => 1,
                        },
                        is_bye: false,
                    },
                    None => NewDbPairingGap {
                        player_id: player.id,
                        tournament_id: self.id,
                        round_id: self.pairings.len() as u32,
                        score: 0,
                        is_bye: false,
                    },
                })
                .collect::<Vec<NewDbPairingGap>>(),
        );
        (db_pairings, db_gaps)
    }
    pub fn current_round(&self) -> usize {
        self.pairings.len()
    }
    pub fn generate_first_round_pairings(
        &self,
        inactive_scores: InactiveScores,
        first_color: Color,
    ) -> Result<NewPairings, AppError> {
        let (mut pairings, byes, floats) = self.prepare_pairings()?;
        // Assign colors in round 1 according to first_color variable
        // Use it to assign the color to the top seed and alternate
        let mut current_color = first_color;
        for pair in pairings.iter_mut() {
            if current_color == Color::White && pair.0 > pair.1 {
                (pair.0, pair.1) = (pair.1, pair.0);
            }
            if current_color == Color::Black && pair.0 < pair.1 {
                (pair.0, pair.1) = (pair.1, pair.0);
            }
            current_color = current_color.other();
        }
        let (pairings, gaps) = self.process_pairings(pairings, byes, inactive_scores);
        Ok(NewPairings {
            round: 0,
            pairings,
            gaps,
            floats,
        })
    }
    pub fn generate_next_round_pairings(
        &self,
        inactive_scores: InactiveScores,
    ) -> Result<NewPairings, AppError> {
        let (mut pairings, byes, floats) = self.prepare_pairings()?;
        // Assing colors in subsequent rounds
        for pair in pairings.iter_mut() {
            let p1 = &self.players[&(pair.0 as u32)];
            let p2 = &self.players[&(pair.1 as u32)];
            let p1_colors = p1.color_history();
            let p2_colors = p2.color_history();
            match (p1_colors.last(), p2_colors.last()) {
                (None, None) => {}
                (None, Some(p2_last_color)) => {
                    if p2_last_color == &Color::Black {
                        pair.0 = p2.id as usize;
                        pair.1 = p1.id as usize;
                    }
                }
                (Some(p1_last_color), None) => {
                    if p1_last_color == &Color::White {
                        pair.0 = p2.id as usize;
                        pair.1 = p1.id as usize;
                    }
                }
                (Some(p1_last_color), Some(p2_last_color)) => {
                    // If players played with different colors in the last round, switch colors for both
                    if p1_last_color != p2_last_color && p1_last_color == &Color::White {
                        // if pair.0 was black last round the order is already correct
                        pair.0 = p2.id as usize;
                        pair.1 = p1.id as usize;
                    }
                    // Both players played with the same color last round
                    if p1_last_color == p2_last_color {
                        // Check for color imbalances in color history
                        let p1_color_balance = p1_colors.iter().fold(0, |acc, c| match c {
                            Color::White => acc + 1,
                            Color::Black => acc - 1,
                        });
                        let p2_color_balance = p2_colors.iter().fold(0, |acc, c| match c {
                            Color::White => acc + 1,
                            Color::Black => acc - 1,
                        });
                        // If pair.0 has more whites he should play as black now
                        if p1_color_balance > p2_color_balance {
                            pair.0 = p2.id as usize;
                            pair.1 = p1.id as usize;
                        }
                        // If both color balances are the same the player with better (lower)
                        // starting rank (tpn) should play as black
                        if p1_color_balance == p2_color_balance
                            && self.player_tpn(p1.id) < self.player_tpn(p2.id)
                        {
                            pair.0 = p2.id as usize;
                            pair.1 = p1.id as usize;
                        }
                    }
                }
            }
        }
        let (pairings, gaps) = self.process_pairings(pairings, byes, inactive_scores);
        if pairings.is_empty() {
            return Err(AppError::EmptyPairingsGenerated);
        }
        Ok(NewPairings {
            round: self.current_round() as u32,
            pairings,
            gaps,
            floats,
        })
    }
    pub fn standings(&self) -> Vec<Vec<PlayerStanding>> {
        let mut standings = Vec::new();
        let mut prev_scores: HashMap<u32, PlayerStanding> = self
            .players
            .keys()
            .map(|id| (*id, PlayerStanding::new(*id)))
            .collect();
        for round in 0..self.current_round() {
            let mut ranking = Vec::new();
            for player in self.players.values() {
                let prev = prev_scores.get(&player.id).unwrap();
                let round_score = match player.history.get(round) {
                    Some(HistoryItem::NotPaired { score }) => *score,
                    Some(HistoryItem::Bye) => 2,
                    Some(HistoryItem::Game {
                        opponent_id: _,
                        color,
                        result,
                    }) => match (color, result) {
                        (Color::White, GameResult::WhiteWins) => 2,
                        (Color::Black, GameResult::BlackWins) => 2,
                        (_, GameResult::Draw) => 1,
                        _ => 0,
                    },
                    _ => 0,
                };
                let mut standing = PlayerStanding::new(player.id);
                standing.score = prev.score + round_score;
                standing.progressive = prev.progressive + standing.score;

                ranking.push(standing);
                prev_scores.entry(player.id).and_modify(|prev| {
                    prev.score += round_score;
                    prev.progressive += standing.progressive;
                });
            }
            for standing in ranking.iter_mut() {
                let player = &self.players[&standing.player_id];
                let opponents: Vec<&Player> = player
                    .history
                    .iter()
                    .take(round as usize + 1)
                    .filter_map(|item| match item {
                        HistoryItem::Game {
                            opponent_id,
                            color: _,
                            result: _,
                        } => self.players.get(opponent_id),
                        _ => None,
                    })
                    .collect();
                let mut opponent_scores: Vec<u32> = opponents
                    .iter()
                    .map(|player| {
                        player
                            .history
                            .iter()
                            .take(round as usize + 1)
                            .map(|item| match item {
                                HistoryItem::NotPaired { score } => *score,
                                HistoryItem::Bye => 2,
                                HistoryItem::Game {
                                    opponent_id: _,
                                    color,
                                    result,
                                } => match (color, result) {
                                    (Color::White, GameResult::WhiteWins) => 2,
                                    (Color::Black, GameResult::BlackWins) => 2,
                                    (_, GameResult::Draw) => 1,
                                    _ => 0,
                                },
                            })
                            .sum()
                    })
                    .collect();
                opponent_scores.sort();
                standing.buchholz = opponent_scores.iter().sum();
                standing.cut_one_buchholz = opponent_scores.iter().skip(1).sum();
                if opponent_scores.pop().is_some() {
                    standing.median_buchholz = opponent_scores.iter().skip(1).sum();
                } else {
                    standing.median_buchholz = 0;
                }
            }
            ranking.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| b.median_buchholz.cmp(&a.median_buchholz))
                    .then_with(|| b.cut_one_buchholz.cmp(&a.cut_one_buchholz))
                    .then_with(|| b.buchholz.cmp(&a.buchholz))
                    .then_with(|| b.progressive.cmp(&a.progressive))
            });
            standings.push(ranking);
        }
        standings
    }
}

pub async fn end_tournament(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    tournament_id: u32,
    claims: Claims,
) -> Result<i64, AppError> {
    let has_permission = check_user_tournament_permissions(pool, tournament_id, claims).await?;
    if !has_permission {
        return Err(AppError::InsufficientPermissions);
    }
    let tournament = get_tournament(pool, tournament_id).await.map_err(|e| {
        tracing::error!("end_tournament (get_tournament): {:?}", e);
        AppError::Unknown
    })?;
    if tournament.current_round < tournament.num_rounds {
        return Err(AppError::CannotEndTournament);
    }
    let pairings = select_pairings(pool, tournament_id).await.map_err(|e| {
        tracing::error!("end_tournament (select_tournament): {:?}", e);
        AppError::Unknown
    })?;
    if pairings
        .iter()
        .map(|p| GameResult::from_str(p.result.as_ref().unwrap_or(&"*".to_string())))
        .any(|r| r == GameResult::Ongoing)
    {
        return Err(AppError::RoundNotDone);
    }
    tournament_repo::end_tournament(pool, tournament_id)
        .await
        .map_err(|e| {
            tracing::error!("end_tournament (end_tournament): {:?}", e);
            AppError::Unknown
        })
}

pub async fn generate_next_pairings(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    tournament_id: u32,
    claims: Claims,
    payload: NextPairings,
) -> Result<NewPairings, AppError> {
    let has_permission = check_user_tournament_permissions(pool, tournament_id, claims).await?;
    if !has_permission {
        return Err(AppError::InsufficientPermissions);
    }
    let scores: InactiveScores = payload.inactive_scores.try_into()?;
    let tournament = read_tournament(pool, tournament_id).await?;
    let tournament: Tournament = tournament.into();
    if tournament.players.len() < 2 {
        return Err(AppError::InsufficientPlayers);
    }
    if tournament.current_round() == 0 {
        let color = match payload.first_color.as_ref().map(|s| s.as_str()) {
            Some("black") => Color::Black,
            Some("white") => Color::White,
            _ => Color::White,
        };
        tournament.generate_first_round_pairings(scores, color)
    } else {
        let round_ongoing = tournament
            .results
            .last()
            .unwrap()
            .iter()
            .any(|r| *r == GameResult::Ongoing);
        if round_ongoing {
            return Err(AppError::RoundNotDone);
        }
        tournament.generate_next_round_pairings(scores)
    }
}

pub async fn update_player_status(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    tournament_id: u32,
    claims: Claims,
    payload: &PlayerStatusPayload,
) -> Result<(), AppError> {
    let has_permission = check_user_tournament_permissions(pool, tournament_id, claims).await?;
    if !has_permission {
        return Err(AppError::InsufficientPermissions);
    }
    let status: PlayerStatus = payload.status.as_str().try_into()?;
    registration_repo::update_registration_status(pool, payload.id, status)
        .await
        .map_err(|e| Into::<AppError>::into(e))
}

pub async fn update_result(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    tournament_id: u32,
    claims: Claims,
    payload: &RoundResult,
) -> Result<(), AppError> {
    let has_permission = check_user_tournament_permissions(pool, tournament_id, claims).await?;
    if !has_permission {
        return Err(AppError::InsufficientPermissions);
    }
    let result = GameResult::from_str(payload.result.clone());
    if result == GameResult::Ongoing {
        tracing::error!("cannot update result to GameResult::Ongoing");
        return Err(AppError::Unknown);
    }
    let tournament = read_tournament(pool, tournament_id).await?;
    let tournament: Tournament = tournament.into();
    if tournament.pairings.is_empty() {
        return Err(AppError::TournamentNotStarted);
    }
    let round = match tournament.results.get(payload.round_id as usize) {
        Some(r) => r,
        None => return Err(AppError::RoundNotFound(payload.round_id as usize)),
    };
    if !round.get(payload.board_id as usize).is_some() {
        tracing::error!("update_result: board id {} not found", payload.board_id);
        return Err(AppError::Unknown);
    }
    if (payload.round_id as usize) < tournament.current_round() - 1 {
        return Err(AppError::InvalidRound(payload.round_id as usize));
    }
    update_game_result(
        pool,
        tournament_id,
        payload.round_id,
        payload.board_id,
        result,
    )
    .await
    .map_err(|e| Into::<AppError>::into(e))
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use crate::models::tournament::{
        Color, GameResult, HistoryItem, Player, PlayerStanding, PlayerStatus, Title, Tournament,
    };

    #[test]
    fn test_standings_basic_no_ties() {
        // Setup a simple tournament with 4 players, 2 rounds, no byes, no ties in scores
        let mut players = HashMap::new();

        // Player 1: Wins both games
        players.insert(
            1,
            Player {
                id: 1,
                db_id: 0,
                name: "Player1".to_string(),
                rating: 2000,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 2,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 3,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 2: Loses first, wins second
        players.insert(
            2,
            Player {
                id: 2,
                db_id: 0,
                name: "Player2".to_string(),
                rating: 1800,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 4,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 3: Wins first, loses second
        players.insert(
            3,
            Player {
                id: 3,
                db_id: 0,
                name: "Player3".to_string(),
                rating: 1900,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 4,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 4: Loses both
        players.insert(
            4,
            Player {
                id: 4,
                db_id: 0,
                name: "Player4".to_string(),
                rating: 1700,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 3,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 2,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        let tournament = Tournament {
            id: 1,
            name: "Test Tournament".to_string(),
            time_category: "Classical".to_string(),
            players,
            pairings: vec![vec![(1, 2), (3, 4)], vec![(1, 3), (2, 4)]], // Dummy pairings, not used in standings
            byes: vec![],
            results: vec![],
            num_rounds: 2,
            start_date: 0,
            federation: "FIDE".to_string(),
            user_id: 0,
            username: "test".to_string(),
            updated_at: 0,
            end_date: None,
            url: None,
        };

        let standings = tournament.standings();

        // After round 1
        // Scores: P1:2 (beat P2), P2:0, P3:2 (beat P4), P4:0
        // Buchholz:
        // P1: opp P2:0 -> 0, cut1:0 (only 1), median:0
        // P2: opp P1:2 -> 2, cut1:0, median:0
        // P3: opp P4:0 -> 0, cut1:0, median:0
        // P4: opp P3:2 -> 2, cut1:0, median:0
        // Sorted by score desc: P1,P3 (2, tie on median 0, cut1 0, buch 0 vs 0, then progressive?)
        // But ignore progressive for testing, assume order P1 then P3 if same.

        // We assert the vec for round 0 (after first round)
        let expected_after_round1 = vec![
            PlayerStanding {
                player_id: 1,
                score: 2,
                buchholz: 0,
                median_buchholz: 0,
                cut_one_buchholz: 0,
                progressive: 0,
            }, // progressive ignored
            PlayerStanding {
                player_id: 3,
                score: 2,
                buchholz: 0,
                median_buchholz: 0,
                cut_one_buchholz: 0,
                progressive: 0,
            },
            PlayerStanding {
                player_id: 2,
                score: 0,
                buchholz: 2,
                median_buchholz: 0,
                cut_one_buchholz: 0,
                progressive: 0,
            },
            PlayerStanding {
                player_id: 4,
                score: 0,
                buchholz: 2,
                median_buchholz: 0,
                cut_one_buchholz: 0,
                progressive: 0,
            },
        ];

        // Note: order may vary if ties, but in code sorts by score then median then cut1 then buch then prog
        // For score 2: both median0, cut0, buch0, then prog
        // But since ignore prog, perhaps set to 0 in expected.

        // After round 2
        // Scores: P1:4 (won again), P2:2, P3:2, P4:0
        // Opponents' scores:
        // P1: P2:2, P3:2 -> [2,2] sum4, sorted[2,2], cut1=2+ (skip1 sum2), median: pop2 now[2], skip1 sum0=0
        // P2: P1:4, P4:0 -> [0,4] sum4, cut1=4 (skip0), median: pop4 [0] skip1=0
        // P3: P4:0, P1:4 -> [0,4] sum4, cut1=4, median:0
        // P4: P3:2, P2:2 -> [2,2] sum4, cut1=2, median:0

        let expected_after_round2 = vec![
            PlayerStanding {
                player_id: 1,
                score: 4,
                buchholz: 4,
                median_buchholz: 0,
                cut_one_buchholz: 2,
                progressive: 0,
            },
            PlayerStanding {
                player_id: 2,
                score: 2,
                buchholz: 4,
                median_buchholz: 0,
                cut_one_buchholz: 4,
                progressive: 0,
            },
            PlayerStanding {
                player_id: 3,
                score: 2,
                buchholz: 4,
                median_buchholz: 0,
                cut_one_buchholz: 4,
                progressive: 0,
            },
            PlayerStanding {
                player_id: 4,
                score: 0,
                buchholz: 4,
                median_buchholz: 0,
                cut_one_buchholz: 2,
                progressive: 0,
            },
        ];

        // For score 2: P2 and P3 have same median0, cut4, buch4, so prog decides order
        // But since ignore, and in expected I have P2 then P3, but may need to sort or check without order

        // To make it simple, assert_eq!(standings[0].len(), 4);
        // But to check values, perhaps sort by player_id or check specific fields

        // For precision, find by id and check fields

        assert_eq!(standings.len(), 2); // two rounds

        // Check after round 1 (index 0)
        let round1 = &standings[0];
        assert_eq!(round1.len(), 4);
        // Sort expected by the sorting criteria, but since prog ignored, perhaps just check values per player

        let mut p1_found = false;
        for standing in round1 {
            if standing.player_id == 1 {
                assert_eq!(standing.score, 2);
                assert_eq!(standing.buchholz, 0);
                assert_eq!(standing.median_buchholz, 0);
                assert_eq!(standing.cut_one_buchholz, 0);
                p1_found = true;
            } else if standing.player_id == 2 {
                assert_eq!(standing.score, 0);
                assert_eq!(standing.buchholz, 2);
                assert_eq!(standing.median_buchholz, 0);
                assert_eq!(standing.cut_one_buchholz, 0);
            } // similarly for 3 and 4
        }
        assert!(p1_found);

        // Similar for round2
        let round2 = &standings[1];
        assert_eq!(round2.len(), 4);
        for standing in round2 {
            match standing.player_id {
                1 => {
                    assert_eq!(standing.score, 4);
                    assert_eq!(standing.buchholz, 4);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 2);
                }
                2 => {
                    assert_eq!(standing.score, 2);
                    assert_eq!(standing.buchholz, 4);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 4);
                }
                3 => {
                    assert_eq!(standing.score, 2);
                    assert_eq!(standing.buchholz, 4);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 4);
                }
                4 => {
                    assert_eq!(standing.score, 0);
                    assert_eq!(standing.buchholz, 4);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 2);
                }
                _ => panic!("Unexpected player"),
            }
        }
    }

    #[test]
    fn test_standings_with_tie_and_buchholz() {
        // Setup with tie in scores, resolved by Buchholz
        let mut players = HashMap::new();

        // Player 1: Draw with 3, win vs 4
        players.insert(
            1,
            Player {
                id: 1,
                db_id: 0,
                name: "Player1".to_string(),
                rating: 2000,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 3,
                        color: Color::White,
                        result: GameResult::Draw,
                    },
                    HistoryItem::Game {
                        opponent_id: 4,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 2: Win vs 4, draw with 3
        players.insert(
            2,
            Player {
                id: 2,
                db_id: 0,
                name: "Player2".to_string(),
                rating: 1800,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 4,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 3,
                        color: Color::Black,
                        result: GameResult::Draw,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 3: Draw with 1, draw with 2
        players.insert(
            3,
            Player {
                id: 3,
                db_id: 0,
                name: "Player3".to_string(),
                rating: 1900,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::Draw,
                    },
                    HistoryItem::Game {
                        opponent_id: 2,
                        color: Color::White,
                        result: GameResult::Draw,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 4: Loss to 2, loss to 1
        players.insert(
            4,
            Player {
                id: 4,
                db_id: 0,
                name: "Player4".to_string(),
                rating: 1700,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 2,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        let tournament = Tournament {
            id: 1,
            name: "Test Tournament".to_string(),
            time_category: "Classical".to_string(),
            players,
            pairings: vec![vec![(1, 3), (2, 4)], vec![(1, 4), (2, 3)]],
            byes: vec![],
            results: vec![],
            num_rounds: 2,
            start_date: 0,
            federation: "FIDE".to_string(),
            user_id: 0,
            username: "test".to_string(),
            updated_at: 0,
            end_date: None,
            url: None,
        };

        let standings = tournament.standings();

        // After round 2
        // Scores: P1:1+2=3, P2:2+1=3, P3:1+1=2, P4:0+0=0
        // Opponents' scores:
        // P1: P3:2, P4:0 -> [0,2] sum2, cut1=2 (skip0), median: pop2 [0] skip1=0 ->0
        // P2: P4:0, P3:2 -> [0,2] sum2, cut1=2, median:0
        // P3: P1:3, P2:3 -> [3,3] sum6, cut1=3, median:0 (pop3 [3] skip1=0)
        // P4: P2:3, P1:3 -> [3,3] sum6, cut1=3, median:0

        // So P1 and P2 tie on score 3, median0, cut1 2, buch2, then prog
        // But buch2 for both

        let round2 = &standings[1];
        for standing in round2 {
            match standing.player_id {
                1 => {
                    assert_eq!(standing.score, 3);
                    assert_eq!(standing.buchholz, 2);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 2);
                }
                2 => {
                    assert_eq!(standing.score, 3);
                    assert_eq!(standing.buchholz, 2);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 2);
                }
                3 => {
                    assert_eq!(standing.score, 2);
                    assert_eq!(standing.buchholz, 6);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 3);
                }
                4 => {
                    assert_eq!(standing.score, 0);
                    assert_eq!(standing.buchholz, 6);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 3);
                }
                _ => panic!("Unexpected player"),
            }
        }
    }

    #[test]
    fn test_standings_with_bye() {
        // 3 players, 2 rounds, with byes
        let mut players = HashMap::new();

        // Player 1: Win vs 2, draw vs 3
        players.insert(
            1,
            Player {
                id: 1,
                db_id: 0,
                name: "Player1".to_string(),
                rating: 2000,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 2,
                        color: Color::White,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Game {
                        opponent_id: 3,
                        color: Color::White,
                        result: GameResult::Draw,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 2: Loss to 1, bye
        players.insert(
            2,
            Player {
                id: 2,
                db_id: 0,
                name: "Player2".to_string(),
                rating: 1800,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::WhiteWins,
                    },
                    HistoryItem::Bye,
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        // Player 3: Bye, draw vs 1
        players.insert(
            3,
            Player {
                id: 3,
                db_id: 0,
                name: "Player3".to_string(),
                rating: 1900,
                title: Title::Untitled,
                history: vec![
                    HistoryItem::Bye,
                    HistoryItem::Game {
                        opponent_id: 1,
                        color: Color::Black,
                        result: GameResult::Draw,
                    },
                ],
                floats: 0,
                fide_id: None,
                federation: None,
                status: PlayerStatus::Active,
            },
        );

        let tournament = Tournament {
            id: 1,
            name: "Test Tournament".to_string(),
            time_category: "Classical".to_string(),
            players,
            pairings: vec![vec![(1, 2)], vec![(1, 3)]], // Dummy, ignoring bye pairs
            byes: vec![vec![3], vec![2]],
            results: vec![],
            num_rounds: 2,
            start_date: 0,
            federation: "FIDE".to_string(),
            user_id: 0,
            username: "test".to_string(),
            updated_at: 0,
            end_date: None,
            url: None,
        };

        let standings = tournament.standings();

        // After round 2
        // Scores: P1:2+1=3, P2:0+2=2, P3:2+1=3
        // Opponents:
        // P1: P2(2), P3(3) -> sum5, [2,3] cut1=3, median: pop3 [2] skip1=0 ->0
        // P2: P1(3), (bye no opp) -> only [3] sum3, cut1=0 (skip1 on1=0), median: pop3 [] 0
        // P3: (bye no), P1(3) -> [3] sum3, cut1=0, median:0

        // So P1 and P3 score3, P1 buch5 > P3 buch3, so P1 higher on buch after median and cut1 same? Median both0, cut1 P1:3 P3:0 so cut1 P1> P3

        let round2 = &standings[1];
        for standing in round2 {
            match standing.player_id {
                1 => {
                    assert_eq!(standing.score, 3);
                    assert_eq!(standing.buchholz, 5);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 3);
                }
                2 => {
                    assert_eq!(standing.score, 2);
                    assert_eq!(standing.buchholz, 3);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 0);
                }
                3 => {
                    assert_eq!(standing.score, 3);
                    assert_eq!(standing.buchholz, 3);
                    assert_eq!(standing.median_buchholz, 0);
                    assert_eq!(standing.cut_one_buchholz, 0);
                }
                _ => panic!("Unexpected player"),
            }
        }
    }
}
