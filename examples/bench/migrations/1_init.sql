CREATE TABLE IF NOT EXISTS test
(
    id   INTEGER NOT NULL
        CONSTRAINT test_pk
            PRIMARY KEY,
    ts   INTEGER NOT NULL,
    name TEXT    NOT NULL
)