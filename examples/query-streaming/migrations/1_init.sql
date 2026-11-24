CREATE TABLE IF NOT EXISTS users
(
    id     INTEGER NOT NULL
        CONSTRAINT users_pk PRIMARY KEY,
    email  TEXT    NOT NULL,
    name   TEXT,
    role   TEXT    NOT NULL,
    logins INTEGER NOT NULL
)
