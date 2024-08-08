CREATE TABLE IF NOT EXISTS test
(
    id          TEXT    NOT NULL
        CONSTRAINT test_pk
            PRIMARY KEY,
    num         INTEGER NOT NULL,
    description TEXT
)