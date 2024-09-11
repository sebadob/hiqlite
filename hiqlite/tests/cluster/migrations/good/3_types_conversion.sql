CREATE TABLE type_conversion
(
    id         INTEGER NOT NULL
        CONSTRAINT type_conversion_pk
            PRIMARY KEY,
    id_none    INTEGER,
    id_opt     INTEGER,
    name       TEXT    NOT NULL,
    name_none  TEXT,
    name_opt   TEXT,
    is_bool    INTEGER NOT NULL,
    utc        TEXT    NOT NULL,
    local      TEXT    NOT NULL,
    offset     TEXT    NOT NULL,
    naive_date TEXT    NOT NULL,
    naive_time TEXT    NOT NULL,
    naive_dt   TEXT    NOT NULL,
    json       TEXT    NOT NULL
);