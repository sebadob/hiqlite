use crate::migration::Migration;
use crate::store::state_machine::sqlite::state_machine;
use crate::store::state_machine::sqlite::state_machine::{
    Params, StateMachineData, StateMachineSqlite, StoredSnapshot,
};
use crate::{Error, Node, NodeId};
use chrono::Utc;
use openraft::{LogId, SnapshotMeta, StorageError, StorageIOError, StoredMembership};
use rusqlite::backup::Progress;
use rusqlite::{Batch, DatabaseName, Transaction};
use std::borrow::Cow;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::{fs, task};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug)]
pub enum WriterRequest {
    SnapshotApply((String, oneshot::Sender<StateMachineData>)),
    Query(Query),
    Migrate(Migrate),
    Snapshot(SnapshotRequest),
    MetadataPersist(MetaPersistRequest),
    MetadataRead(oneshot::Sender<Vec<u8>>),
}

#[derive(Debug)]
pub enum Query {
    Execute(SqlExecute),
    Transaction(SqlTransaction),
    Batch(SqlBatch),
}

#[derive(Debug)]
pub struct SqlExecute {
    pub sql: Cow<'static, str>,
    pub params: Params,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub tx: oneshot::Sender<Result<usize, Error>>,
}

#[derive(Debug)]
pub struct SqlTransaction {
    pub queries: Vec<state_machine::Query>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub tx: oneshot::Sender<Result<Vec<Result<usize, Error>>, Error>>,
}

#[derive(Debug)]
pub struct SqlBatch {
    pub sql: Cow<'static, str>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub tx: oneshot::Sender<Vec<Result<usize, Error>>>,
}

#[derive(Debug)]
pub struct Migrate {
    pub migrations: Vec<Migration>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub tx: oneshot::Sender<Result<(), Error>>,
}

#[derive(Debug)]
pub struct SnapshotRequest {
    pub snapshot_id: Uuid,
    pub last_membership: StoredMembership<NodeId, Node>,
    pub path: String,
    pub ack: oneshot::Sender<Result<SnapshotResponse, StorageError<NodeId>>>,
}

#[derive(Debug)]
pub struct SnapshotResponse {
    pub meta: StateMachineData,
}

#[derive(Debug)]
pub struct MetaPersistRequest {
    pub data: Vec<u8>,
    pub ack: flume::Sender<()>,
}

pub fn spawn_writer(
    // path_db: String,
    // filename_db: String,
    mut conn: rusqlite::Connection,
    // in_memory: bool,
) -> flume::Sender<WriterRequest> {
    let (tx, rx) = flume::bounded::<WriterRequest>(2);

    let _handle = thread::spawn(move || {
        let mut last_applied_log_id: Option<LogId<NodeId>> = None;

        // TODO before doing anything else, apply migrations

        // we want to handle our metadata manually to not interfere with migrations in apps later on
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS _metadata
            (
                key  TEXT    NOT NULL
                    CONSTRAINT _metadata_pk
                        PRIMARY KEY,
                data BLOB    NOT NULL
            )"#,
            (),
        )
        .expect("_metadata table creation to always succeed");

        loop {
            let req = match rx.recv() {
                Ok(r) => r,
                Err(err) => {
                    // TODO This should actually only ever happen on shutdown.
                    // TODO remove after enough testing
                    error!("SQLite write handler channel recv error: {:?}", err);
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
            };
            debug!("Query in Write handler: {:?}", req);

            match req {
                WriterRequest::Query(query) => match query {
                    Query::Execute(q) => {
                        let res = {
                            let mut stmt = match conn.prepare_cached(q.sql.as_ref()) {
                                Ok(stmt) => stmt,
                                Err(err) => {
                                    error!("Preparing cached query {}: {:?}", q.sql, err);
                                    q.tx.send(Err(Error::PrepareStatement(err.to_string().into())))
                                        .expect("oneshot tx to never be dropped");
                                    continue;
                                }
                            };

                            // let params_len = q.params.len();
                            let mut params_err = None;
                            let mut idx = 1;
                            for param in q.params {
                                if let Err(err) = stmt.raw_bind_parameter(idx, param.into_sql()) {
                                    error!(
                                        "Error binding param on position {} to query {}: {:?}",
                                        idx, q.sql, err
                                    );
                                    params_err = Some(Error::QueryParams(err.to_string().into()));
                                    break;
                                }

                                idx += 1;
                            }

                            if let Some(err) = params_err {
                                q.tx.send(Err(err)).expect("oneshot tx to never be dropped");
                                continue;
                            }

                            stmt.raw_execute().map_err(Error::from)
                        };

                        last_applied_log_id = q.last_applied_log_id;
                        q.tx.send(res).expect("oneshot tx to never be dropped");
                    }

                    Query::Transaction(req) => {
                        let txn = match conn.transaction() {
                            Ok(txn) => txn,
                            Err(err) => {
                                error!("Opening database transaction: {:?}", err);
                                req.tx
                                    .send(Err(Error::Transaction(err.to_string().into())))
                                    .expect("oneshot tx to never be dropped");
                                continue;
                            }
                        };

                        let mut results = Vec::with_capacity(req.queries.len());
                        'outer: for state_machine::Query { sql, params } in req.queries {
                            let mut stmt = match txn.prepare_cached(sql.as_ref()) {
                                Ok(stmt) => stmt,
                                Err(err) => {
                                    error!("Preparing cached query {}: {:?}", sql, err);
                                    results
                                        .push(Err(Error::PrepareStatement(err.to_string().into())));
                                    continue;
                                }
                            };

                            let mut idx = 1;
                            for param in params {
                                if let Err(err) = stmt.raw_bind_parameter(idx, param.into_sql()) {
                                    error!(
                                        "Error binding param on position {} to query {}: {:?}",
                                        idx, sql, err
                                    );
                                    results.push(Err(Error::QueryParams(err.to_string().into())));
                                    continue 'outer;
                                }

                                idx += 1;
                            }

                            let res = stmt.raw_execute().map_err(Error::from);
                            results.push(res);
                        }

                        last_applied_log_id = req.last_applied_log_id;

                        match txn.commit() {
                            Ok(()) => {
                                req.tx
                                    .send(Ok(results))
                                    .expect("oneshot tx to never be dropped");
                            }
                            Err(err) => {
                                req.tx
                                    .send(Err(Error::Transaction(err.to_string().into())))
                                    .expect("oneshot tx to never be dropped");
                            }
                        }
                    }

                    Query::Batch(req) => {
                        let mut batch = Batch::new(&conn, req.sql.as_ref());
                        // we can at least assume 2 statements in a batch execute
                        let mut res = Vec::with_capacity(2);

                        loop {
                            match batch.next() {
                                Ok(Some(mut stmt)) => {
                                    res.push(stmt.execute([]).map_err(Error::from));
                                }
                                Ok(None) => break,
                                // will happen if the query can't be prepared -> syntax error
                                Err(err) => {
                                    res.push(Err(Error::from(err)));
                                }
                            }
                        }

                        last_applied_log_id = req.last_applied_log_id;
                        req.tx.send(res).expect("oneshot tx to never be dropped");
                    }
                },

                WriterRequest::Migrate(req) => {
                    let res = migrate(&mut conn, req.migrations).map_err(Error::from);

                    // TODO should be maybe always panic if migrations throw an error?

                    last_applied_log_id = req.last_applied_log_id;
                    req.tx.send(res).unwrap();
                }

                WriterRequest::Snapshot(SnapshotRequest {
                    snapshot_id,
                    path,
                    last_membership,
                    ack,
                }) => {
                    match create_snapshot(
                        &conn,
                        snapshot_id,
                        path,
                        last_applied_log_id,
                        last_membership,
                    ) {
                        Ok(meta) => ack.send(Ok(SnapshotResponse { meta })),
                        Err(err) => {
                            error!("Error creating new snapshot: {:?}", err);
                            ack.send(Err(StorageError::IO {
                                source: StorageIOError::write(&err),
                            }))
                        }
                    }
                    .expect("snapshot listener to always exists");
                }

                WriterRequest::SnapshotApply((path, ack)) => {
                    let start = Instant::now();
                    info!("Starting snapshot restore from {}", path);
                    conn.restore(
                        DatabaseName::Main,
                        path,
                        Some(|p: Progress| {
                            println!("Database restore remaining: {}", p.remaining);
                        }),
                    )
                    .expect("SnapshotApply to always succeed in sql writer");
                    info!(
                        "Snapshot restore finished after {} ms",
                        start.elapsed().as_millis()
                    );

                    let metadata = conn
                        .query_row("SELECT data FROM _metadata WHERE key = 'meta'", (), |row| {
                            let meta_bytes: Vec<u8> = row.get(0)?;
                            let metadata: StateMachineData = bincode::deserialize(&meta_bytes)
                                .expect("Metadata to deserialize ok");
                            Ok(metadata)
                        })
                        .expect("Metadata query to always succeed");

                    ack.send(metadata).unwrap()
                }

                WriterRequest::MetadataPersist(MetaPersistRequest { data, ack }) => {
                    let mut stmt = conn
                        .prepare_cached("REPLACE INTO _metadata (key, data) VALUES ('meta', $1)")
                        .expect("Metadata persist prepare to never fail");

                    stmt.execute([data])
                        .expect("Metadata persist to never fail");

                    ack.send(()).unwrap();
                }

                WriterRequest::MetadataRead(ack) => {
                    let mut stmt = conn
                        .prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")
                        .expect("Metadata read prepare to always succeed");

                    let meta = stmt
                        .query_row((), |row| {
                            let bytes: Vec<u8> = row.get(0)?;
                            Ok(bytes)
                        })
                        .expect("Database to always have at least default metadata");

                    ack.send(meta).unwrap();
                }
            }
        }
    });

    tx
}

fn create_snapshot(
    conn: &rusqlite::Connection,
    snapshot_id: Uuid,
    path: String,
    last_applied_log_id: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, Node>,
) -> Result<StateMachineData, rusqlite::Error> {
    let metadata = StateMachineData {
        last_applied_log_id,
        last_membership,
        last_snapshot_id: Some(snapshot_id.to_string()),
        last_snapshot_path: Some(path.clone()),
    };

    let meta_bytes = bincode::serialize(&metadata).unwrap();
    let mut stmt = conn.prepare("UPDATE _metadata SET data = $1 WHERE key = 'meta'")?;
    stmt.execute([meta_bytes])?;

    let q = format!("VACUUM main INTO '{}'", path);
    conn.execute(&q, ())?;

    Ok(metadata)
}

fn migrate(conn: &mut rusqlite::Connection, mut migrations: Vec<Migration>) -> Result<(), Error> {
    info!("Applying database migrations");

    create_migrations_table(conn)?;

    let mut last_applied = last_applied_migration(conn, &migrations)?;
    debug!("Last applied migration: {}", last_applied);
    migrations.retain(|m| m.id > last_applied);
    debug!("Leftover migrations to apply: {:?}", migrations);

    for migration in migrations {
        if migration.id != last_applied + 1 {
            panic!(
                "Migration index has a gap between {} and {}",
                last_applied, migration.id
            );
        }
        last_applied = migration.id;

        let txn = conn.transaction()?;
        apply_migration(txn, migration)?;
    }

    Ok(())
}

#[inline]
fn create_migrations_table(conn: &rusqlite::Connection) -> Result<(), Error> {
    conn.execute(
        r#"
    CREATE TABLE IF NOT EXISTS _migrations
    (
        id   INTEGER    NOT NULL
            CONSTRAINT _migrations_pk
                PRIMARY KEY,
        name TEXT       NOT NULL,
        ts   INTEGER    NOT NULL,
        hash TEXT       NOT NULL
    )
    "#,
        [],
    )?;

    Ok(())
}

/// Validates the already applied migrations against the given ones and returns the
/// start index for new to apply migrations, if everything was ok.
#[inline]
fn last_applied_migration(
    conn: &rusqlite::Connection,
    migrations: &[Migration],
) -> Result<u32, Error> {
    let mut stmt = conn.prepare("SELECT * FROM _migrations")?;
    let already_applied: Vec<Migration> = stmt
        .query_map([], |row| {
            Ok(Migration {
                id: row.get(0)?,
                name: row.get(1)?,
                hash: row.get(2)?,
                content: row.get(3)?,
            })
        })?
        .map(|r| r.expect("_migrations table corrupted"))
        .collect();

    let mut last_applied = 0;
    for applied in already_applied {
        if last_applied + 1 != applied.id {
            panic!(
                "Applied migrations order mismatch: expected {}, got {}",
                last_applied + 1,
                applied.id
            );
        }
        last_applied = applied.id;

        match migrations.get(last_applied as usize - 1) {
            None => panic!("Missing migration with id {}", last_applied),
            Some(migration) => {
                if applied.id != migration.id {
                    panic!(
                        "Migration id mismatch: applied {}, given {}",
                        applied.id, migration.id
                    );
                }

                if applied.name != migration.name {
                    panic!(
                        "Name for migration {} has changed: applied {}, given {}",
                        migration.id, applied.name, migration.name
                    );
                }

                if applied.hash != migration.hash {
                    panic!(
                        "Hash for migration {} has changed: applied {}, given {}",
                        migration.id, applied.hash, migration.hash
                    );
                }
            }
        }
    }

    Ok(last_applied)
}

#[inline]
fn apply_migration(txn: rusqlite::Transaction, migration: Migration) -> Result<(), Error> {
    info!(
        "Applying database migration {} {}",
        migration.id, migration.name
    );

    let sql = String::from_utf8_lossy(&migration.content);
    let mut batch = Batch::new(&txn, &sql);

    while let Some(mut stmt) = batch.next()? {
        stmt.execute([])?;
    }

    {
        let mut stmt = txn.prepare(
            r#"
        INSERT INTO _migrations (id, name, ts, hash)
        VALUES ($1, $2, $3, $4)
        "#,
        )?;
        stmt.execute((
            migration.id,
            migration.name,
            Utc::now().timestamp(),
            migration.hash,
        ))?;
    }

    txn.commit()?;
    Ok(())
}