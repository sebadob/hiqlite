CREATE TABLE complex
(
    id         INTEGER NOT NULL
        CONSTRAINT complex_pk
            PRIMARY KEY,
    -- renamed via attribute
    name_db    TEXT    NOT NULL,
    desc       TEXT,
    vec_wrap   TEXT,
    some_int   INTEGER NOT NULL,
    left_right TEXT    NOT NULL,
    ud         TEXT    NOT NULL,
    num        INTEGER NOT NULL,

    -- these will be mapped to our `EntitySub`
    sub_id     INTEGER NOT NULL,
    sub_name   TEXT    NOT NULL,

    -- will be mapped to `EntitySubSub`
    secret     TEXT    NULL,

    -- will be mapped to `MyEnum`
    enum_value TEXT    NOT NULL
) STRICT;
