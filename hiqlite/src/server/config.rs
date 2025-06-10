use crate::helpers::{read_line_stdin, set_path_access};
use crate::server::args::{ArgsConfig, ArgsGenerate};
use crate::server::password;
use crate::{Error, NodeConfig};
use cryptr::{utils, EncKeys};
use tokio::fs;

pub async fn build_node_config(args: ArgsConfig) -> Result<NodeConfig, Error> {
    let config_path = if args.config_file == "$HOME/.hiqlite/hiqlite.toml" {
        default_config_file_path()
    } else {
        args.config_file
    };
    let mut config = NodeConfig::from_toml(&config_path, None, None).await?;

    if let Some(id) = args.node_id {
        config.node_id = id;
    }
    if let Some(log) = args.log_statements {
        config.log_statements = log;
    }

    Ok(config)
}

pub async fn generate(args: ArgsGenerate) -> Result<(), Error> {
    let path = default_config_dir();
    fs::create_dir_all(&path).await?;
    set_path_access(&path, 0o700).await?;

    let path_file = default_config_file_path();
    if fs::File::open(&path_file).await.is_ok() {
        eprint!(
            "Config file {} exists already. Overwrite? (yes): ",
            path_file
        );
        let line = read_line_stdin().await?;
        if line != "yes" {
            return Ok(());
        }
    }

    let pwd_plain = if args.password {
        let plain;
        loop {
            println!("Provide a password with at least 16 characters: ");
            let line = read_line_stdin().await?;
            if line.len() >= 16 {
                plain = line;
                break;
            } else {
                eprintln!("Input too short - has only {} characters", line.len());
            }
        }
        plain
    } else {
        utils::secure_random_alnum(24)
    };
    println!("New password for the dashboard: {}", pwd_plain);
    let password_dashboard = password::hash_password_b64(pwd_plain).await?;

    let default_config = default_config(&password_dashboard, args.insecure_cookie)?;
    fs::write(&path_file, default_config).await?;
    println!("New default config file created: {}", path_file);

    set_path_access(&path_file, 0o600).await?;

    Ok(())
}

#[inline]
fn home_dir() -> String {
    let home = home::home_dir().expect("Cannot get current $HOME");
    home.to_str()
        .expect("Invalid characters in $HOME")
        .to_string()
}

#[inline]
fn default_config_dir() -> String {
    format!("{}/.hiqlite", home_dir())
}

#[inline]
fn default_config_file_path() -> String {
    format!("{}/hiqlite.toml", default_config_dir())
}

fn default_config(password_dashboard_b64: &str, insecure_cookie: bool) -> Result<String, Error> {
    let data_dir = format!("{}/data", default_config_dir());
    let secret_raft = utils::secure_random_alnum(32);
    let secret_api = utils::secure_random_alnum(32);
    let enc_keys = EncKeys::generate()?;
    let enc_keys_b64 = enc_keys.keys_as_b64_vec();
    let enc_key = enc_keys_b64.first().unwrap();
    let enc_key_active = enc_keys.enc_key_active;

    Ok(format!(
        r#"[hiqlite]
# Can be set to 'k8s' to try to split off the node id from the hostname
# when Hiqlite is running as a StatefulSet inside Kubernetes.
#
# default: unset
# overwritten by: HQL_NODE_ID_FROM
#node_id_from = "k8s"

# The node id must exist in the nodes and there must always be
# at least a node with ID 1
# Will be ignored if `node_id_from = k8s`
#
# At least `node_id_from` or `node_id` are required.
#
# default: 0 (invalid)
# overwritten by: HQL_NODE_ID
node_id = 1

# All cluster member nodes.
# Each array value must have the following format:
# `id addr_raft addr_api`
#
# default: ["1 localhost:8100 localhost:8200"]
# overwritten by: HQL_NODES
nodes = [
    "1 localhost:8100 localhost:8200"
]

# The data dir hiqlite will store raft logs and state machine data in.
#
# default: data
# overwritten by: HQL_DATA_DIR
data_dir = "{data_dir}"

# The file name of the SQLite database in the state machine folder.
#
# default: hiqlite.db
# overwritten by: HQL_FILENAME_DB
#filename_db = "hiqlite.db"

# If set to `true`, all SQL statements will be logged for debugging
# purposes.
#
# default: false
# overwritten by: HQL_LOG_STATEMENTS
#log_statements = false

# The size of the internal cache for prepared statements.
#
# default: 1000
#prepared_statement_cache_capacity = 1000

# The size of the pooled connections for local database reads.
#
# Do not confuse this with a pool size for network databases, as it
# is much more efficient. You can't really translate between them,
# because it depends on many things, but assuming a factor of 10 is
# a good start. This means, if you needed a (read) pool size of 40
# connections for something like a postgres before, you should start
# at a `read_pool_size` of 4.
#
# Keep in mind that this pool is only used for reads and writes will
# travel through the Raft and have their own dedicated connection.
#
# default: 4
# overwritten by: HQL_READ_POOL_SIZE
#read_pool_size = 4

# Setting for Raft Log syncing / flushing.
#
# This value is probably the most important, depending on your needs.
# It has a huge impact on both performance and consistency.
#
# You can set 3 different levels:
# - immediate
# - immediate_async
# - interval_<time_ms> (e.g. interval_200)
#
# If you run a single instance "Cluster", you most probably want
# `immediate` to have the highest degree of consistency. If set
# to `immediate`, the Raft will block until data has been flushed
# to disk. This is especially important for a single instance
# deployment, because there is no way to recover state from other
# nodes or re-sync a maybe corrupted WAL file.
# `immediate` has a very huge negative impact on throughput, and
# it puts a lot of stress on your disk, which is important to
# consider for SSDs.
#
# `immediate_async` puts the same amount of stress on your SSD
# and flushed all buffers to disk immediately after a single
# Raft Log was saved. Unlike `immediate` however, it does not
# wait for completion and directly returns `success` to the
# Raft Engine. You have a bit less consistency guarantees in
# exchange for basically the same throughput as with `interval`
# syncing.
#
# The `interval_<ms>` option will not flush immediately. Flushes
# will be triggered by an external ticker task top flush buffers
# every <ms> ms, and then only if buffers are dirty, meaning if
# any data has been updated since the last sync. This setting
# has the highest throughput and puts the lowest amount of stress
# on your SSD.
# CAUTION: You probably never want to use this setting for a
# single instance, since it may be the case that if you are
# unlucky and your app crashes before buffers are synced, that
# your WAL file might end up being corrupted. Even though very
# unlikely in real world (can be force-produced though), you
# would need to re-sync from a healthy cluster member in such
# a case, if the automactic WAL repair does not succeed.
#
# default: immediate_async
# overwritten by: HQL_LOG_SYNC
#log_sync = "immediate_async"

# Hiqlite WAL files (when not using the `rocksdb` feature) will
# always have a fixed size, even when they are still "empty", to
# reduce disk operations while writing. You can set the WAL size
# in bytes. The default value is 2 MB, while the minimum size is
# 8 kB.
#
# default: 2097152
# overwritten by: HQL_WAL_SIZE
#wal_size = 2097152

# Set to `false` to store Cache WAL files + Snapshots in-memory only.
# If you run a Cluster, a Node can re-sync cache data after a restart.
# However, if you restart too quickly or shut down the whole cluster,
# all your cached data will be gone.
# In-memory only hugegly increases the throughput though, so it
# depends on your needs, what you should prefer.
#
# default: true
# overwritten by: HQL_CACHE_STORAGE_DISK
#cache_storage_disk = true

# Sets the limit when the Raft will trigger the creation of a new
# state machine snapshot and purge all logs that are included in
# the snapshot.
# Higher values can achieve more throughput in very write heavy
# situations but will end up in more disk usage and longer
# snapshot creations / log purges.
#
# default: 10000
# overwritten by: HQL_LOGS_UNTIL_SNAPSHOT
#logs_until_snapshot = 10000

# The artificial shutdown delay to add in multi-node environments.
# This value is being added before finally shutting down a node
# to make sure rolling releases can be executed graceful with
# proper leader switches. You may want to increase this value if
# you are in an environment with huge in-memory caches and you
# want to provide a bit more headroom for the snapshot replication
# after restarts.
#
# default: 5000
# overwritten by: HQL_SHUTDOWN_DELAY_MILLS
#shutdown_delay_millis = 5000

# If given, these keys / certificates will be used to establish
# TLS connections between nodes.
#
# values are optional, overwritten by: HQL_TLS_{{RAFT|API}}_{{KEY|CERT}}
#tls_raft_key = "tls/key.pem"
#tls_raft_cert = "tls/cert-chain.pem"
#tls_raft_danger_tls_no_verify = true

#tls_api_key = "tls/key.pem"
#tls_api_cert = "tls/cert-chain.pem"
#tls_api_danger_tls_no_verify = true

# Secrets for Raft internal authentication as well as for the API.
# These must be at least 16 characters long and you should provide
# different ones for both variables.
#
# default: not set - required
# overwritten by: HQL_SECRET_RAFT
secret_raft = "{secret_raft}"
# default: not set - required
# overwritten by: HQL_SECRET_API
secret_api = "{secret_api}"

# Configures the initial delay in seconds that should be applied
# to `<API>/health` checks. During the first X seconds after node
# start, health checks will always return true to solve a chicken-
# and-egg problem when you want to cold-start a cluster while
# relying on `readinessProbe` checks.
#
# default: 30
#health_check_delay_secs = 30

# When the auto-backup task should run.
# Accepts cron syntax:
# "sec min hour day_of_month month day_of_week year"
#
# default: "0 30 2 * * * *"
# overwritten by: HQL_BACKUP_CRON
#backup_cron = "0 30 2 * * * *"

# Backups older than the configured days will be cleaned up on S3
# after the backup cron job `backup_cron`.
#
# default: 30
# overwritten by: HQL_BACKUP_KEEP_DAYS
#backup_keep_days = 30

# Backups older than the configured days will be cleaned up locally
# after each `Client::backup()` and the cron job `HQL_BACKUP_CRON`.
#
# default: 3
# overwritten by: HQL_BACKUP_KEEP_DAYS_LOCAL
#backup_keep_days_local = 3

# If you ever need to restore from a backup, the process is simple.
# 1. Have the cluster shut down. This is probably the case anyway, if
#    you need to restore from a backup.
# 2. Provide a backup file name on S3 storage with the
#    `HQL_BACKUP_RESTORE` value with prefix `s3:` (encrypted), or a file
#    on disk (plain sqlite file) with the prefix `file:`.
# 3. Start up the cluster again.
# 4. After the restart, make sure to remove the HQL_BACKUP_RESTORE
#    env value.
#
# CAUTION: can only be set via ENV VAR temporarily
#HQL_BACKUP_RESTORE=

# The Hiqlite backup restore process checks the `_metadata` table
# in backups as an integrity check. If you however want to "restore"
# from an already existing default SQLite file, you can disable
# this validation to make the restore process succeed anyway.
# To do so, set `HQL_BACKUP_SKIP_VALIDATION=true`.
#
# CAUTION: can only be set via ENV VAR temporarily
#HQL_BACKUP_SKIP_VALIDATION=

# Access values for the S3 bucket where backups will be pushed to.
# overwritten by: HQL_S3_URL
#s3_url = "https://s3.example.com"
# overwritten by: HQL_S3_BUCKET
#s3_bucket = "my_bucket"
# overwritten by: HQL_S3_REGION
#s3_region = "my_region"
# overwritten by: HQL_S3_PATH_STYLE
#s3_path_style = true
# overwritten by: HQL_S3_KEY
#s3_key = "my_key"
# overwritten by: HQL_S3_SECRET
#s3_secret = "my_secret"

# You need to define at least one valid encryption key.
# These keys are used to encrypt the database backups that will
# be pushed to S3 storage.
#
# The format must match (\n separated for env var, array in toml):
# ENC_KEYS="
# q6u26onRvXVG4427/M0NFQzhSSldCY01rckJNa1JYZ3g2NUFtSnNOVGdoU0E=
# bVCyTsGaggVy5yqQ/UzluN29DZW41M3hTSkx6Y3NtZmRuQkR2TnJxUTYzcjQ=
# "
#
# The first part until the first `/` is the key ID.
# The ID must match '[a-zA-Z0-9]{{2,20}}'
#
# The key itself begins after the first `/` has been found.
# The key must be exactly 32 bytes long, encoded as base64.
#
# You can find a more detailed explanation on how to generate
# keys in the Rauthy documentation:
# https://sebadob.github.io/rauthy/config/encryption.html
#
# You can provide multiple keys to make things like key
# rotation work. Be careful with removing old keys. Make sure
# that all secrets have been migrated beforehand.
# You can find a utility in the Admin UI to do this for you.
#
# default: not set - mandatory value
# overwritten by: ENC_KEYS
enc_keys = [ "{enc_key}" ]

# This identifies the key ID from the `ENC_KEYS` list, that
# should actively be used for new encryptions.
#
# default: not set - mandatory value
# overwritten by: ENC_KEY_ACTIVE
enc_key_active = "{enc_key_active}"

# The password for the dashboard as b64 encoded Argon2ID hash.
# If left empty, the Dashboard will not be exposed.
#
# '123SuperMegaSafe' in this example
# overwritten by: HQL_PASSWORD_DASHBOARD
password_dashboard = "{password_dashboard_b64}"

# Can be set to `true` during local dev and testing to issue
# insecure cookies
#
# default: false
# overwritten by: HQL_INSECURE_COOKIE
insecure_cookie = {insecure_cookie}

# Can be set to true to start the WAL handler even if the
# `lock.hql` file exists. This may be necessary after a
# crash, when the lock file could not be removed during a
# graceful shutdown.
#
# IMPORTANT: Even though the Database can "heal" itself by
# simply rebuilding from the existing Raft log files without
# even needing to think about it, you may want to only
# set `HQL_WAL_IGNORE_LOCK` when necessary to have more
# control. Depending on the type of crash (whole OS, maybe
# immediate power loss, force killed, ...), it may be the
# case that the WAL files + metadata could not be synced
# to disk properly and that quite a bit of data is lost.
#
# In such a case, it is usually a better idea to delete
# the whole volume and let the broken node rebuild from
# other healthy cluster members, just to be sure.
#
# However, you can decide to ignore the lock file and start
# anyway. But you must be 100% sure, that no orphaned
# process is still running and accessing the WAL files!
#
# default: false
# overwritten by: HQL_WAL_IGNORE_LOCK
#wal_ignore_lock = false

# You can reset the Raft Logs + Metadata when set to
# `true`. This can be helpful, if you e.g. run a single
# instance "cluster" and encountered an unrecoverable
# error at startup, like e.g. a corrupted Raft Logs volume.
# If you have an HA cluster though, in such a situation,
# a volume cleanup for the broken node and clean cluster
# re-join + sync is always preferred and the safer option.
#
# Another situation where this option can be used, if you
# want to trigger a Log ID roll-over and for some reason,
# you either cannot or do not want to apply a backup to
# achieve this. This may only be necessary if your Log ID
# almost reached `18446744073709551615`, which in reality
# will probably never happen in less than 10 years.
# Applying a backup at that point is still the safer
# option, because it is not possible to make a mistake,
# as long as you wait for it to finish.
#
# Be VERY CAREFUL with this option! If used incorrectly,
# you can destroy a Raft cluster and end up with an
# inconsistent state, so your only chance last chance
# is to either re-create the whole cluster, or apply a
# backup.
#
# This option will leave the state machine in place,
# but delete Raft WAL, Metadata and Snapshots!
# Local backups will be left in place.
#
# This option should be seen as a last resort.
# USE WITH CARE!
#
# !!!
# This value only exists here for completeness.
# You can only set it via ENV to be able to remove it
# immediately again after a start.
# !!!
#
# default: false
#HQL_DANGER_RAFT_STATE_RESET=true
"#
    ))
}
