create table users (
    id integer primary key autoincrement,
    username text unique not null,
    email text unique,
    password_hash text not null,
    role text not null check (role in ('standard', 'admin')),
    created_at integer default (unixepoch()) not null
);

create table tournaments (
    id integer not null primary key autoincrement,
    name text not null,
    time_category text not null,
    current_round integer not null,
    num_rounds integer not null,
    start_date integer not null,
    federation text not null,
    created_by integer not null,
    updated_at integer DEFAULT (unixepoch()) not null,
    end_date integer,
    url text,
    constraint fk_tournament_owner foreign key (created_by) references users(id)
);

create table players (
    id integer not null primary key autoincrement,
    first_name text not null,
    last_name text not null,
    updated_at integer not null,
    federation text,
    fide_id integer unique,
    title text,
    rating integer,
    rating_rapid integer,
    rating_blitz integer
);

create table registrations (
    id integer not null primary key autoincrement,
    player_id integer not null,
    tournament_id integer not null,
    floats integer not null,
    status text not null,
    rating integer not null,
    constraint fk_registration_player foreign key (player_id) references players(id),
    constraint fk_registration_tournament foreign key (tournament_id) references tournaments(id),
    constraint uq_registration unique (player_id, tournament_id)
);

create table pairings (
    id integer not null primary key autoincrement,
    tournament_id integer not null,
    round_number integer not null,
    board_number integer not null,
    white_id integer not null,
    black_id integer not null,
    result text,
    pgn text,
    constraint fk_pairing_tournament foreign key (tournament_id) references tournaments(id),
    constraint fk_pairing_white foreign key (white_id) references registrations(id),
    constraint fk_pairing_black foreign key (black_id) references registrations(id),
    constraint ck_white_diff_black check (white_id != black_id),
    constraint uq_pairing unique (tournament_id, round_number, board_number, white_id, black_id)
);

create table pairing_gaps (
    id integer not null primary key autoincrement,
    player_id integer not null,
    tournament_id integer not null,
    round_id integer not null,
    score integer not null,
    is_bye boolean not null,
    constraint fk_pgap_player foreign key (player_id) references registrations(id),
    constraint fk_pgap_tournament foreign key (tournament_id) references tournaments(id)
);
