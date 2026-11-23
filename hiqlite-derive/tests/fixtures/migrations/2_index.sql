CREATE INDEX test_ts ON test (ts);

INSERT INTO test (id, ts, description)
VALUES (1, 10, 'one'),
       (2, 20, NULL);
