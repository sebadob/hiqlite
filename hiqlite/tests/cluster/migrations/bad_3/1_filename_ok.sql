-- this file name is good
-- it should throw an error anyway because the second sql statement has a syntax error

CREATE TABLE bad_1
(
    id          INTEGER NOT NULL
        CONSTRAINT test_pk
            PRIMARY KEY,
    ts          INTEGER NOT NULL,
    description TEXT    NOT NULL
);

CREATE TABLE bad_2
(
    id          INTEGER NOT NULL
        CONSTRAINT test_pk
            PRIMARY KEY,
    ts          INTEGER NOT NULL,
    description TEXT    NOT NULL,
);
