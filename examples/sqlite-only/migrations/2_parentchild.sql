CREATE TABLE IF NOT EXISTS parent
(
    id          INTEGER PRIMARY KEY,
    description TEXT
);

CREATE TABLE IF NOT EXISTS child
(
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER NOT NULL REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED,
    description TEXT
);
