use crate::migration::Migration;
use crate::query::rows::{ColumnOwned, RowOwned, ValueOwned};
use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine;
use crate::store::state_machine::sqlite::state_machine::{
    Params, StateMachineData, StateMachineSqlite, StoredSnapshot,
};
use crate::{AppliedMigration, Error, Node, NodeId};
use chrono::Utc;
use flume::RecvError;
use openraft::{LogId, SnapshotMeta, StorageError, StorageIOError, StoredMembership};
use rusqlite::backup::Progress;
use rusqlite::{Batch, DatabaseName, Transaction};
use std::borrow::Cow;
use std::default::Default;
use std::ops::Sub;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::{fs, task};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug)]
pub enum WriterRequest {
    Query(Query),
    Migrate(Migrate),
    Snapshot(SnapshotRequest),
    SnapshotApply((String, oneshot::Sender<()>)),
    // SnapshotApply((String, oneshot::Sender<StateMachineData>)),
    // MetadataPersist(MetaPersistRequest),
    MetadataRead(oneshot::Sender<StateMachineData>),
    MetadataMembership(MetaMembershipRequest),
    Backup(BackupRequest),
    // BackupApply(BackupApplyRequest),
    Shutdown(oneshot::Sender<()>),
    #[allow(clippy::upper_case_acronyms)]
    RTT(RTTRequest),
}

#[derive(Debug)]
pub enum Query {
    Execute(SqlExecute),
    ExecuteReturning(SqlExecuteReturning),
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
pub struct SqlExecuteReturning {
    pub sql: Cow<'static, str>,
    pub params: Params,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub tx: oneshot::Sender<Result<Vec<Result<RowOwned, Error>>, Error>>,
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
    pub tx: oneshot::Sender<Result<Vec<Result<usize, Error>>, Error>>,
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
    pub data: StateMachineData,
    // pub data: Vec<u8>,
    pub ack: flume::Sender<()>, // TODO flume only needed for sync `drop()` -> convert to oneshot after being fixed
}

#[derive(Debug)]
pub struct MetaMembershipRequest {
    pub last_membership: StoredMembership<NodeId, Node>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub ack: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct BackupRequest {
    pub node_id: NodeId,
    pub target_folder: String,
    #[cfg(feature = "s3")]
    pub s3_config: Option<std::sync::Arc<crate::s3::S3Config>>,
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub ack: oneshot::Sender<Result<(), Error>>,
}

#[derive(Debug)]
pub struct RTTRequest {
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub ack: oneshot::Sender<()>,
}

// #[derive(Debug)]
// pub struct BackupApplyRequest {
//     pub src: String,
//     pub ack: oneshot::Sender<Result<(), Error>>,
// }

#[allow(clippy::blocks_in_conditions)]
pub fn spawn_writer(
    mut conn: rusqlite::Connection,
    this_node: NodeId,
    path_lock_file: String,
    log_statements: bool,
) -> flume::Sender<WriterRequest> {
    let (tx, rx) = flume::bounded::<WriterRequest>(2);

    // thread::spawn(move || {
    task::spawn_blocking(move || {
        let mut sm_data = StateMachineData::default();
        let mut ts_last_backup = None;

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

        'main: while let Ok(req) = rx.recv() {
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

                    Query::ExecuteReturning(q) => {
                        sm_data.last_applied_log_id = q.last_applied_log_id;

                        if log_statements {
                            info!("Query::ExecuteReturning:\n{}\n{:?}", q.sql, q.params);
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

                            let columns = match ColumnOwned::mapping_cols_from_stmt(stmt.columns())
                            {
                                Ok(c) => c,
                                Err(err) => {
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

                            let mut rows = stmt.raw_query();
                            let mut res = Vec::new();
                            loop {
                                match rows.next() {
                                    Ok(Some(row)) => {
                                        res.push(Ok(RowOwned::from_row_column(row, &columns)));
                                    }
                                    Ok(None) => {
                                        break;
                                    }
                                    Err(err) => {
                                        res.push(Err(Error::Sqlite(err.to_string().into())));
                                    }
                                }
                            }

                            Ok(res)
                        };

                        q.tx.send(res).expect("oneshot tx to never be dropped");
                    }

                    Query::Transaction(req) => {
                        sm_data.last_applied_log_id = req.last_applied_log_id;

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
                        let mut query_err = None;

                        'outer: for state_machine::Query { sql, params } in req.queries {
                            if log_statements {
                                info!("Query::Transaction:\n{}\n{:?}", sql, params);
                            }

                            let mut stmt = match txn.prepare_cached(sql.as_ref()) {
                                Ok(stmt) => stmt,
                                Err(err) => {
                                    let err = format!("Preparing cached query {}: {:?}", sql, err);
                                    query_err =
                                        Some(Error::PrepareStatement(err.to_string().into()));
                                    break;
                                }
                            };

                            let mut idx = 1;
                            for param in params {
                                if let Err(err) = stmt.raw_bind_parameter(idx, param.into_sql()) {
                                    let err = format!(
                                        "Error binding param on position {} to query {}: {:?}",
                                        idx, sql, err
                                    );
                                    query_err = Some(Error::QueryParams(err.to_string().into()));
                                    break 'outer;
                                }

                                idx += 1;
                            }

                            let res = stmt.raw_execute().map_err(Error::from);
                            match res {
                                Ok(r) => results.push(Ok(r)),
                                Err(err) => {
                                    query_err = Some(Error::Transaction(err.to_string().into()));
                                    break;
                                }
                            }
                        }

                        if let Some(err) = query_err {
                            if let Err(e) = txn.rollback() {
                                error!("Error during txn rollback: {:?}", e);
                            }
                            req.tx
                                .send(Err(err))
                                .expect("oneshot tx to never be dropped");
                        } else {
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
                    }

                    Query::Batch(req) => {
                        sm_data.last_applied_log_id = req.last_applied_log_id;

                        if log_statements {
                            info!("Query::Batch:\n{}", req.sql);
                        }

                        let mut batch = Batch::new(&conn, req.sql.as_ref());
                        // we can at least assume 2 statements in a batch execute
                        let mut res = Vec::with_capacity(2);

                        let mut err = None;

                        loop {
                            match batch.next() {
                                Ok(Some(mut stmt)) => {
                                    res.push(stmt.execute([]).map_err(Error::from));
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    // The `Batch` iterator can't recover from errors
                                    // -> exit early and do not commit the txn
                                    err = Some(Error::Sqlite(e.to_string().into()));
                                    break;
                                }
                            }
                        }

                        if let Some(err) = err {
                            req.tx
                                .send(Err(err))
                                .expect("oneshot tx to never be dropped");
                        } else {
                            req.tx
                                .send(Ok(res))
                                .expect("oneshot tx to never be dropped");
                        }
                    }
                },

                WriterRequest::Migrate(req) => {
                    sm_data.last_applied_log_id = req.last_applied_log_id;

                    // TODO should be maybe always panic if migrations throw an error?
                    let res = migrate(&mut conn, req.migrations).map_err(Error::from);

                    if let Err(err) = conn.execute("PRAGMA optimize", []) {
                        error!("Error during 'PRAGMA optimize': {}", err);
                    }

                    req.tx.send(res).unwrap();
                }

                WriterRequest::Snapshot(SnapshotRequest {
                    snapshot_id,
                    path,
                    // last_membership,
                    ack,
                }) => {
                    sm_data.last_snapshot_id = Some(snapshot_id.to_string());
                    persist_metadata(&conn, &sm_data).expect("Metadata persist to never fail");

                    match create_snapshot(
                        &conn,
                        // snapshot_id,
                        path,
                        // sm_data.last_applied_log_id,
                        // sm_data.last_membership.clone(),
                    ) {
                        Ok(_) => {
                            if let Err(err) = conn.execute("PRAGMA optimize", []) {
                                error!("Error during 'PRAGMA optimize': {}", err);
                            }

                            ack.send(Ok(SnapshotResponse {
                                meta: sm_data.clone(),
                            }))
                        }
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

                    if let Err(err) = conn.execute("PRAGMA optimize", []) {
                        error!("Error during 'PRAGMA optimize': {}", err);
                    }

                    info!(
                        "Snapshot restore finished after {} ms",
                        start.elapsed().as_millis()
                    );

                    sm_data = conn
                        .query_row("SELECT data FROM _metadata WHERE key = 'meta'", (), |row| {
                            let meta_bytes: Vec<u8> = row.get(0)?;
                            let metadata: StateMachineData = bincode::deserialize(&meta_bytes)
                                .expect("Metadata to deserialize ok");
                            Ok(metadata)
                        })
                        .expect("Metadata query to always succeed");

                    ack.send(()).unwrap()
                }

                WriterRequest::MetadataRead(ack) => {
                    if sm_data.last_applied_log_id.is_none() {
                        let mut stmt = conn
                            .prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")
                            .expect("Metadata read prepare to always succeed");

                        match stmt.query_row((), |row| {
                            let bytes: Vec<u8> = row.get(0)?;
                            Ok(bytes)
                        }) {
                            Ok(bytes) => {
                                sm_data = bincode::deserialize(&bytes).unwrap();
                            }
                            Err(err) => {
                                warn!("No metadata exists inside the DB yet");
                            }
                        }
                    }

                    ack.send(sm_data.clone()).unwrap();
                }

                WriterRequest::MetadataMembership(req) => {
                    sm_data.last_membership = req.last_membership;
                    sm_data.last_applied_log_id = req.last_applied_log_id;
                    req.ack.send(()).unwrap();
                }

                WriterRequest::Backup(req) => {
                    sm_data.last_applied_log_id = req.last_applied_log_id;

                    // TODO include a TS in the req to skip backups if they are replayed after
                    // a restart
                    let now = Utc::now();
                    if let Some(ts) = ts_last_backup {
                        if ts > now.sub(chrono::Duration::seconds(60)) {
                            info!("Received duplicate backup request within the last 60 seconds - ignoring it");
                            req.ack.send(Ok(()));
                            continue;
                        }
                    }

                    info!("VACUUMing the database");
                    let start = Instant::now();
                    match conn.execute("VACUUM", ()) {
                        Ok(_) => {
                            info!("VACUUM finished after {} ms", start.elapsed().as_millis());
                        }
                        Err(err) => error!("Error during VACUUM: {}", err),
                    }

                    // only the current leader should push the backup
                    #[cfg(feature = "s3")]
                    let s3_config = if this_node == req.node_id {
                        req.s3_config
                    } else {
                        None
                    };

                    if let Err(err) = create_backup(
                        &conn,
                        req.node_id,
                        req.target_folder.clone(),
                        #[cfg(feature = "s3")]
                        s3_config,
                    ) {
                        error!("Error creating backup: {:?}", err);
                        req.ack.send(Err(err));
                        continue;
                    }

                    #[cfg(feature = "backup")]
                    task::spawn(async move {
                        if let Err(err) =
                            crate::backup::backup_local_cleanup(req.target_folder).await
                        {
                            error!("Error during local backup cleanup: {:?}", err);
                        }
                    });

                    if let Err(err) = conn.execute("PRAGMA optimize", []) {
                        error!("Error during 'PRAGMA optimize': {}", err);
                    }

                    ts_last_backup = Some(now);
                    req.ack.send(Ok(()));
                }

                WriterRequest::RTT(req) => {
                    sm_data.last_applied_log_id = req.last_applied_log_id;
                    req.ack.send(()).unwrap();
                }

                WriterRequest::Shutdown(ack) => {
                    let _ = ack.send(());
                    break;
                }
            }
        }

        warn!("SQL writer is shutting down");

        // make sure metadata is persisted before shutting down
        persist_metadata(&conn, &sm_data).expect("Error persisting metadata");

        if let Err(err) = conn.execute("PRAGMA optimize", []) {
            error!("Error during 'PRAGMA optimize': {}", err);
        }

        StateMachineSqlite::remove_lock_file(&path_lock_file);
    });

    tx
}

#[inline]
fn persist_metadata(
    conn: &rusqlite::Connection,
    metadata: &StateMachineData,
) -> Result<(), rusqlite::Error> {
    let meta_bytes = bincode::serialize(metadata).unwrap();
    let mut stmt = conn.prepare("REPLACE INTO _metadata (key, data) VALUES ('meta', $1)")?;
    stmt.execute([meta_bytes])?;
    Ok(())
}

#[inline]
fn create_snapshot(conn: &rusqlite::Connection, path: String) -> Result<(), rusqlite::Error> {
    let q = format!("VACUUM main INTO '{}'", path);
    conn.execute(&q, ())?;
    Ok(())
}

fn create_backup(
    conn: &rusqlite::Connection,
    node_id: NodeId,
    target_folder: String,
    #[cfg(feature = "s3")] s3_config: Option<std::sync::Arc<crate::s3::S3Config>>,
) -> Result<(), Error> {
    // - build target db file name with node id and timestamp
    // - vacuum into target file
    // - connect to vacuumed db and reset metadata
    // - if we have an s3 target, encrypt and push it

    let file = format!("backup_node_{}_{}.sqlite", node_id, Utc::now().timestamp());
    let path_full = format!("{}/{}", target_folder, file);
    info!("Creating database backup into {}", path_full);

    conn.execute(&format!("VACUUM main INTO '{}'", path_full), ())?;

    // connect to the backup and reset metadata
    // make sure connection is dropped before starting encrypt + push
    {
        let conn_bkp = rusqlite::Connection::open(&path_full)?;
        persist_metadata(&conn_bkp, &StateMachineData::default());
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
    debug!(
        "Leftover migrations to apply: {:?}",
        migrations.iter().map(|m| format!("{}_{}", m.id, m.name))
    );

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
    if migrations.is_empty() {
        return Err(Error::Error("Received empty migrations".into()));
    }

    // We need the first id to skip all other existing migrations in the DB.
    // The client is optimized to reduce requests and strip out already existing ones.
    let first_id = migrations.first().as_ref().unwrap().id;

    if first_id > 1 {
        // double check, that we actually have the correct amount of migrations already applied.
        let mut stmt = conn.prepare("SELECT COUNT(*) AS count FROM _migrations WHERE id < $1")?;
        let count: u32 = stmt.query_row([first_id], |row| {
            let count: u32 = row.get("count")?;
            Ok(count)
        })?;
        if count < first_id - 1 {
            panic!("Received optimized migrations starting at id '{}' but found only {} already applied", first_id, count);
        }
    }

    let mut stmt = conn.prepare("SELECT * FROM _migrations WHERE id >= $1 ORDER BY id ASC")?;
    let already_applied: Vec<AppliedMigration> = stmt
        .query_map([first_id], |row| {
            Ok(AppliedMigration {
                id: row.get(0)?,
                name: row.get(1)?,
                ts: row.get(2)?,
                hash: row.get(3)?,
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
                        "Migration id mismatch: applied {}, given {}\n{:?}",
                        applied.id, migration.id, migrations
                    );
                }

                if applied.name != migration.name {
                    panic!(
                        "Name for migration {} has changed: applied {}, given {}\n{:?}",
                        migration.id, applied.name, migration.name, migrations
                    );
                }

                if applied.hash != migration.hash {
                    panic!(
                        "Hash for migration {} has changed: applied {}, given {}\n{:?}",
                        migration.id, applied.hash, migration.hash, migrations
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
