use std::{collections::HashMap, fmt::Display};

use serde::Serialize;

use crate::{
    errors::AppError,
    repositories::{
        pairing_repo::{DbPairing, DbPairingGap, NewDbPairing, NewDbPairingGap},
        registration_repo::DbRegistration,
        tournament_repo::DbTournament,
    },
};

pub struct TournamentDbData {
    pub tournament: DbTournament,
    pub players: Vec<DbRegistration>,
    pub pairings: Vec<DbPairing>,
    pub pairing_gaps: Vec<DbPairingGap>,
}

#[derive(Debug)]
pub struct Tournament {
    pub id: u32,
    pub name: String,
    pub time_category: String,
    pub players: HashMap<u32, Player>,
    pub pairings: Vec<Vec<(usize, usize)>>,
    pub byes: Vec<Vec<u32>>,
    pub results: Vec<Vec<GameResult>>,
    pub num_rounds: usize,
    pub start_date: usize,
    pub federation: String,
    pub user_id: u32,
    pub username: String,
    pub updated_at: u32,
    pub end_date: Option<u32>,
    pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    Draw,
    BlackWins,
    DoubleLoss,
}

impl GameResult {
    pub fn from_str<S: AsRef<str>>(str: S) -> Self {
        match str.as_ref().trim() {
            "1-0" => Self::WhiteWins,
            "1 - 0" => Self::WhiteWins,
            "1/2-1/2" => Self::Draw,
            "1/2 - 1/2" => Self::Draw,
            "½-½" => Self::Draw,
            "½ - ½" => Self::Draw,
            "=-=" => Self::Draw,
            "= - =" => Self::Draw,
            "0-1" => Self::BlackWins,
            "0 - 1" => Self::BlackWins,
            "0-0" => Self::DoubleLoss,
            "0 - 0" => Self::DoubleLoss,
            _ => Self::Ongoing,
        }
    }
}

impl Display for GameResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameResult::Ongoing => write!(f, "*"),
            GameResult::WhiteWins => write!(f, "1-0"),
            GameResult::Draw => write!(f, "=-="),
            GameResult::BlackWins => write!(f, "0-1"),
            GameResult::DoubleLoss => write!(f, "0-0"),
        }
    }
}

#[derive(Debug)]
pub enum PlayerResult {
    Win,
    Lose,
    Draw,
}

impl PlayerResult {
    pub fn from_str<S: AsRef<str>>(str: S) -> Self {
        match str.as_ref().trim() {
            "win" => Self::Win,
            "draw" => Self::Draw,
            _ => Self::Lose,
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PlayerStatus {
    #[default]
    Active,
    Inactive,
}

impl TryFrom<&str> for PlayerStatus {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "inactive" => Ok(Self::Inactive),
            "active" => Ok(Self::Active),
            _ => Err(AppError::InvalidPlayerStatus(value.to_owned())),
        }
    }
}

impl PlayerStatus {
    pub fn from_str<S: AsRef<str>>(str: S) -> Self {
        match str.as_ref().trim() {
            "inactive" => Self::Inactive,
            _ => Self::Active,
        }
    }
}

impl Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerStatus::Active => write!(f, "active"),
            PlayerStatus::Inactive => write!(f, "inactive"),
        }
    }
}

#[derive(Default, Debug)]
pub struct Player {
    pub id: u32,
    pub db_id: u32,
    pub name: String,
    pub rating: u32,
    pub title: Title,
    pub history: Vec<HistoryItem>,
    pub floats: usize,
    pub fide_id: Option<usize>,
    pub federation: Option<String>,
    pub status: PlayerStatus,
}

impl Player {
    pub fn color_history(&self) -> Vec<Color> {
        self.history
            .iter()
            .filter_map(|item| match item {
                HistoryItem::NotPaired { score: _ } => None,
                HistoryItem::Bye => None,
                HistoryItem::Game {
                    opponent_id: _,
                    color,
                    result: _,
                } => Some(color.to_owned()),
            })
            .collect()
    }

    pub fn has_played(&self, player_id: u32) -> bool {
        self.history
            .iter()
            .filter_map(|item| match item {
                HistoryItem::NotPaired { score: _ } => None,
                HistoryItem::Bye => None,
                HistoryItem::Game {
                    opponent_id,
                    color: _,
                    result: _,
                } => Some(opponent_id),
            })
            .any(|id| *id == player_id)
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum HistoryItem {
    NotPaired {
        score: u32,
    },
    Bye,
    Game {
        opponent_id: u32,
        color: Color,
        result: GameResult,
    },
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Title {
    #[default]
    Untitled,
    WNM,
    WCM,
    WFM,
    NM,
    CM,
    WIM,
    FM,
    WGM,
    IM,
    GM,
}

impl Title {
    pub fn from_str<S: AsRef<str>>(str: S) -> Self {
        match str.as_ref().to_lowercase().trim() {
            "wnm" => Self::WNM,
            "woman National Master" => Self::WNM,
            "wcm" => Self::WCM,
            "woman Candidate Master" => Self::WCM,
            "wfm" => Self::WFM,
            "woman FIDE Master" => Self::WFM,
            "nm" => Self::NM,
            "national Master" => Self::NM,
            "cm" => Self::CM,
            "candidate Master" => Self::CM,
            "wim" => Self::WIM,
            "woman International Master" => Self::WIM,
            "fm" => Self::FM,
            "fide Master" => Self::FM,
            "wgm" => Self::WGM,
            "woman Grandmaster" => Self::WGM,
            "im" => Self::IM,
            "international Master" => Self::IM,
            "gm" => Self::GM,
            "grandmaster" => Self::GM,
            _ => Self::Untitled,
        }
    }
}

impl Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Title::Untitled => write!(f, ""),
            Title::WNM => write!(f, "WNM"),
            Title::WCM => write!(f, "WCM"),
            Title::WFM => write!(f, "WFM"),
            Title::NM => write!(f, "NM"),
            Title::CM => write!(f, "CM"),
            Title::WIM => write!(f, "WIM"),
            Title::FM => write!(f, "FM"),
            Title::WGM => write!(f, "WGM"),
            Title::IM => write!(f, "IM"),
            Title::GM => write!(f, "GM"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn other(&self) -> Self {
        match self {
            Color::White => Self::Black,
            Color::Black => Self::White,
        }
    }
}

pub struct NewPairings {
    pub round: u32,
    pub pairings: Vec<NewDbPairing>,
    pub gaps: Vec<NewDbPairingGap>,
    pub floats: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStanding {
    pub player_id: u32,
    pub score: u32,
    pub buchholz: u32,
    pub median_buchholz: u32,
    pub cut_one_buchholz: u32,
    pub progressive: u32,
}

impl PlayerStanding {
    pub fn new(id: u32) -> Self {
        Self {
            player_id: id,
            score: 0,
            buchholz: 0,
            median_buchholz: 0,
            cut_one_buchholz: 0,
            progressive: 0,
        }
    }
}
