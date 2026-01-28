# Swiss Matching - Backend

Rust-powered RESTful API for managing **Swiss-system chess tournaments**.

This is the **backend** repository (SQLite-based).  
The frontend (SolidJS application) is in a separate repo:  
**[https://github.com/RoJac88/swiss-matching-fe]**.

## Features

- Swiss-system pairing engine
- Tournament CRUD (create, read, update, delete)
- Player management:
  - Fetch & cache FIDE player data by ID (ratings, title, name, federation, etc.)
  - Custom player registration
  - Persistent player database (reusable across tournaments)
- Late joins / withdrawals supported
- Result submission & automatic scoring (1-0, ½-½, 0-1, forfeits)
- Public read-only endpoints for tournament state (pairings, standings, results)
- Authentication & authorization:
  - JWT-based auth
  - Only tournament creator can edit/delete
  - Public access for viewing finished/running tournaments
- Optional admin user auto-creation on startup (via `ADMIN_USERNAME` + `ADMIN_PASSWORD`)
- Rate limiting (TODO), input validation, error handling

## Tech Stack

- **Language**: Rust (stable channel)
- **Web Framework**: [Axum](https://github.com/tokio-rs/axum) (async, ergonomic)
- **Database**: SQLite (via [sqlx](https://github.com/launchbadge/sqlx))
- **Queries**: sqlx for async operations
- **Authentication**: JWT (jsonwebtoken crate)
- **Serialization**: serde + serde_json
- **Logging**: tracing + tracing-subscriber
- **CORS**: tower-http
- **Other crates**:
  - chrono (dates/times)
  - reqwest (FIDE API fetching)
  - thiserror / anyhow (error handling)
  - argon2 (password hashing)

## Quick Start

### Prerequisites

- Rust ≥ 1.75 (stable)
- Cargo (comes with Rust)

### Development

1. Clone the repo

   ```bash
   git clone https://github.com/yourusername/swiss-matching.git
   cd swiss-matching
   ```

2. Copy & configure .env

  ```bash
  cp .env.example .env
  ```

3. Setup database

  ```bash
  sqlx database create
  sqlx migrate run
  ```

4. Run the server

  ```bash
  cargo run
  ```
