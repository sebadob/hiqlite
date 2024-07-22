use crate::migration::Migration;
use crate::store::logs;
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
use std::default::Default;
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
    MetadataRead(oneshot::Sender<StateMachineData>),
    MetadataMembership(MetaMembershipRequest),
    Backup(BackupRequest),
    Shutdown(oneshot::Sender<()>),
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
    // pub last_membership: StoredMembership<NodeId, Node>,
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
    pub ack: flume::Sender<()>, // TODO flume only needed for sync `drop()` -> convert to oneshot after being fixed
}

#[derive(Debug)]
pub struct MetaMembershipRequest {
    pub last_membership: StoredMembership<NodeId, Node>,
    pub ack: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct BackupRequest {
    pub node_id: NodeId,
    pub target_folder: String,
    #[cfg(feature = "s3")]
    pub s3_config: Option<std::sync::Arc<crate::S3Config>>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub ack: oneshot::Sender<Result<(), Error>>,
}

pub fn spawn_writer(
    mut conn: rusqlite::Connection,
    path_lock_file: String,
    log_statements: bool,
) -> flume::Sender<WriterRequest> {
    let (tx, rx) = flume::bounded::<WriterRequest>(2);

    task::spawn_blocking(move || {
        let mut sm_data = StateMachineData::default();

        // TODO should we maybe save a backup task handle in case of shutdown overlap?

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
                        sm_data.last_applied_log_id = q.last_applied_log_id;

                        if log_statements {
                            info!("Query::Execute:\n{}\n{:?}", q.sql, q.params);
                        }

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

                        sm_data.last_applied_log_id = req.last_applied_log_id;

                        let mut results = Vec::with_capacity(req.queries.len());
                        'outer: for state_machine::Query { sql, params } in req.queries {
                            if log_statements {
                                info!("Query::Transaction:\n{}\n{:?}", sql, params);
                            }

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
                        if log_statements {
                            info!("Query::Batch:\n{}", req.sql);
                        }

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

                        sm_data.last_applied_log_id = req.last_applied_log_id;
                        req.tx.send(res).expect("oneshot tx to never be dropped");
                    }
                },

                WriterRequest::Migrate(req) => {
                    sm_data.last_applied_log_id = req.last_applied_log_id;

                    // TODO should be maybe always panic if migrations throw an error?
                    let res = migrate(&mut conn, req.migrations).map_err(Error::from);
                    req.tx.send(res).unwrap();
                }

                WriterRequest::Snapshot(SnapshotRequest {
                    snapshot_id,
                    path,
                    // last_membership,
                    ack,
                }) => {
                    match create_snapshot(
                        &conn,
                        snapshot_id,
                        path,
                        sm_data.last_applied_log_id,
                        sm_data.last_membership.clone(),
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

                WriterRequest::Shutdown(ack) => {
                    let mut stmt = conn
                        .prepare_cached("REPLACE INTO _metadata (key, data) VALUES ('meta', $1)")
                        .expect("Metadata persist prepare to never fail");

                    let data = bincode::serialize(&sm_data).unwrap();
                    stmt.execute([data])
                        .expect("Metadata persist to never fail");

                    StateMachineSqlite::remove_lock_file(&path_lock_file);

                    ack.send(()).unwrap();

                    info!("Received shutdown signal. Metadata persisted successfully.");
                    break;
                }

                WriterRequest::MetadataRead(ack) => {
                    let mut stmt = conn
                        .prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")
                        .expect("Metadata read prepare to always succeed");

                    let bytes = stmt
                        .query_row((), |row| {
                            let bytes: Vec<u8> = row.get(0)?;
                            Ok(bytes)
                        })
                        .expect("Database to always have at least default metadata");

                    let meta: StateMachineData = bincode::deserialize(&bytes).unwrap();
                    ack.send(meta).unwrap();
                }

                WriterRequest::MetadataMembership(req) => {
                    sm_data.last_membership = req.last_membership;
                    req.ack.send(()).unwrap();
                }

                WriterRequest::Backup(req) => {
                    sm_data.last_applied_log_id = req.last_applied_log_id;

                    match create_backup(
                        &conn,
                        req.node_id,
                        req.target_folder,
                        #[cfg(feature = "s3")]
                        req.s3_config,
                    ) {
                        Ok(meta) => req.ack.send(Ok(())),
                        Err(err) => {
                            error!("Error creating backup: {:?}", err);
                            req.ack.send(Err(err))
                        }
                    }
                    .expect("snapshot listener to always exists");
                }
            }
        }

        warn!("SQL writer is shutting down");
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

fn create_backup(
    conn: &rusqlite::Connection,
    node_id: NodeId,
    target_folder: String,
    #[cfg(feature = "s3")] s3_config: Option<std::sync::Arc<crate::S3Config>>,
) -> Result<(), Error> {
    // TODO
    // - build target db file name with node id and timestamp
    // - vacuum into target file
    // - connect to vacuumed db and remove metadata
    // - if we have an s3 target, encrypt and push it

    let file = format!("backup_node_{}_{}.sqlite", node_id, Utc::now().timestamp());
    let path_full = format!("{}/{}", target_folder, file,);
    info!("Creating database backup into {}", path_full);

    conn.execute(&format!("VACUUM main INTO '{}'", path_full), ())?;

    // connect to the backup and reset metadata
    // make sure connection is dropped before starting encrypt + push
    {
        let conn_bkp = rusqlite::Connection::open(&path_full)?;
        let mut stmt =
            conn_bkp.prepare("REPLACE INTO _metadata (key, data) VALUES ('meta', $1)")?;
        let data = bincode::serialize(&StateMachineData::default()).unwrap();
        stmt.execute([data])?;
    }

    info!("Database backup finished");

    #[cfg(feature = "s3")]
    if let Some(s3) = s3_config {
        task::spawn(async move {
            info!("Background task for database encryption and S3 backup task has been started");

            match s3.push(&path_full, &file).await {
                Ok(_) => {
                    info!("Push backup to S3 has been finished");
                }
                Err(err) => {
                    error!("Error pushing Backup to S3: {}", err);
                }
            }
        });
    }

    Ok(())
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
