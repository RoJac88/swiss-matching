use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("The authenticaton header is missing or invalid")]
    InvalidAuthHeader,
    #[error("The provided jwt is invalid or has expired, please reauthenticate")]
    TokenInvalid,
    #[error("Cannot end tournament with remaining rounds to go")]
    CannotEndTournament,
    #[error("Insufficient permissions to perform this action")]
    InsufficientPermissions,
    #[error("Username already exists: {0}")]
    UsernameTaken(String),
    #[error("Login Failed: {0}")]
    LoginFailed(String),
    #[error("Unknown JSON Error")]
    JsonUnknownError,
    #[error("Missing JSON content-type header")]
    MissingContentType,
    #[error("JSON Syntax error: {0}")]
    JsonSyntaxError(String),
    #[error("Invalid JSON data")]
    JsonDataError,
    #[error("Failed get info from FIDE: {0}")]
    FideScrapeFailed(String),
    #[error("Not enough players registered")]
    InsufficientPlayers,
    #[error("No valid pairings available, failed to generate next round pairings")]
    EmptyPairingsGenerated,
    #[error("Invalid player status: `{0}, possible values are: active and inactive`")]
    InvalidPlayerStatus(String),
    #[error("Duplicate player result for id: `{0}`, only one score per player is allowed")]
    DuplicatePlayerResult(u32),
    #[error("Cannot generate next round pairings if there are still ongoing games")]
    RoundNotDone,
    #[error("Invalid player id: `{0}`")]
    InvalidPlayerId(u32),
    #[error("Invalid score: `{0}, possible values are: win, lose and draw`")]
    InvalidPlayerScore(String),
    #[error("Time category `{0}` is not valid, possible values are: blitz, rapid and standard")]
    InvalidTimeCategory(String),
    #[error("Cannot create tournament with `{0}` rounds, must be between 2 and 30")]
    InvalidNumberOfRounds(u32),
    #[error("Tournament round `{0}` does not exist")]
    RoundNotFound(usize),
    #[error("Game {game:?}, from round {round:?} does not exist")]
    GameNotFound { round: usize, game: usize },
    #[error("Player with id `{0}` does not exist")]
    PlayerNotFound(usize),
    #[error("Cannot skip a round when inserting game history")]
    InsertGameHistorySkipsRound,
    #[error("Cannot execute action after tournament has ended")]
    TournamentEnded,
    #[error("Cannot execute action before tournament has started")]
    TournamentNotStarted,
    #[error("No tournament found with the provided id")]
    TournamentNotFound,
    #[error("Invalid action for round `{0}`")]
    InvalidRound(usize),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("unknown error")]
    Unknown,
}

impl AppError {
    pub fn code(&self) -> String {
        match self {
            AppError::RoundNotFound(_) => String::from("RoundNotFound"),
            AppError::GameNotFound { round: _, game: _ } => String::from("GameNotFound"),
            AppError::PlayerNotFound(_) => String::from("PlayerNotFound"),
            AppError::InsertGameHistorySkipsRound => String::from("InsertGameHistorySkipsRound"),
            AppError::TournamentEnded => String::from("TournamentEnded"),
            AppError::InvalidRound(_) => String::from("InvalidRound"),
            AppError::Unknown => String::from("Unknown"),
            AppError::Database(_) => String::from("DatabaseError"),
            AppError::InvalidTimeCategory(_) => String::from("InvalidTimeCategory"),
            AppError::InvalidNumberOfRounds(_) => String::from("InvalidNumberOfRounds"),
            AppError::DuplicatePlayerResult(_) => String::from("DuplicatePlayerResult"),
            AppError::InvalidPlayerId(_) => String::from("InvalidPlayerId"),
            AppError::InvalidPlayerScore(_) => String::from("InvalidPlayerScore"),
            AppError::TournamentNotStarted => String::from("TournamentNotStarted"),
            AppError::RoundNotDone => String::from("RoundNotDone"),
            AppError::InvalidPlayerStatus(_) => String::from("InvalidPlayerStatus"),
            AppError::EmptyPairingsGenerated => String::from("EmptyPairingsGenerated"),
            AppError::InsufficientPlayers => String::from("InsufficientPlayers"),
            AppError::FideScrapeFailed(_) => String::from("FideScrapeFailed"),
            AppError::MissingContentType => String::from("MissingContentType"),
            AppError::JsonSyntaxError(_) => String::from("JsonSyntaxErro"),
            AppError::JsonDataError => String::from("JsonDataError"),
            AppError::JsonUnknownError => String::from("JsonUnknownError"),
            AppError::LoginFailed(_) => String::from("LoginFailed"),
            AppError::UsernameTaken(_) => String::from("UsernameTaken"),
            AppError::TournamentNotFound => String::from("TournamentNotFound"),
            AppError::InsufficientPermissions => String::from("InsufficientPermissions"),
            AppError::CannotEndTournament => String::from("CannotEndTournament"),
            AppError::TokenInvalid => String::from("TokenInvalid"),
            AppError::InvalidAuthHeader => String::from("InvalidAuthHeader"),
        }
    }
}
