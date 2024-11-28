use crate::helpers::{fn_access, read_line_stdin};
use crate::server::args::{ArgsConfig, ArgsGenerate};
use crate::server::password;
use crate::{Error, NodeConfig};
use cryptr::{utils, EncKeys};
use tokio::{fs, task};

pub fn build_node_config(args: ArgsConfig) -> Result<NodeConfig, Error> {
    let config_path = if args.config_file == "$HOME/.hiqlite/config" {
        default_config_file_path()
    } else {
        args.config_file
    };
    let mut config = NodeConfig::from_env_all(&config_path);

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
    fn_access(&path, 0o700).await?;

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
            if line.len() > 16 {
                plain = line;
                break;
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

    fn_access(&path_file, 0o600).await?;

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
    format!("{}/config", default_config_dir())
}

fn default_config(password_dashboard_b64: &str, insecure_cookie: bool) -> Result<String, Error> {
    let data_dir = format!("{}/data", default_config_dir());
    let secret_raft = utils::secure_random_alnum(32);
    let secret_api = utils::secure_random_alnum(32);
    let enc_keys = EncKeys::generate()?;
    let enc_keys_b64 = enc_keys.keys_as_b64()?;
    let enc_keys_trimmed = enc_keys_b64.trim();
    let enc_key_active = enc_keys.enc_key_active;

    Ok(format!(
        r#"# Can be set to 'k8s' to try to split off the node id from the hostname
# when Hiqlite is running as a StatefulSet inside Kubernetes.
#HQL_NODE_ID_FROM=k8s

# The node id must exist in the nodes and there must always be
# at least a node with ID 1.
# Will be ignored if `HQL_NODE_ID_FROM=k8s`
HQL_NODE_ID=1

# All cluster member nodes.
# To make setting the env var easy, the values are separated by `\s`
# while nodes are separated by `\n`
# in the following format:
#
# id addr_raft addr_api
# id addr_raft addr_api
# id addr_raft addr_api
#
#HQL_NODES="
#1 localhost:8100 localhost:8200
#2 localhost:8100 localhost:8200
#3 localhost:8100 localhost:8200
#"
HQL_NODES="
1 localhost:8100 localhost:8200
"

# The data dir hiqlite will store raft logs and state machine data in.
# default: hiqlite_data
HQL_DATA_DIR={data_dir}

# The file name of the SQLite database in the state machine folder.
# default: hiqlite.db
#HQL_FILENAME_DB=hiqlite.db

# If set to `true`, all SQL statements will be logged for debugging
# purposes.
# default: false
#HQL_LOG_STATEMENTS=false

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
#HQL_READ_POOL_SIZE=4

# Enables immediate flush + sync to disk after each Log Store Batch.
# The situations where you would need this are very rare, and you
# should use it with care.
#
# The default is `false`, and a flush + sync will be done in 200ms
# intervals. Even if the application should crash, the OS will take
# care of flushing left-over buffers to disk and no data will get
# lost. If something worse happens, you might lose the last 200ms
# of commits (on that node, not the whole cluster). This is only
# important to know for single instance deployments. HA nodes will
# sync data from other cluster members after a restart anyway.
#
# The only situation where you might want to enable this option is
# when you are on a host that might lose power out of nowhere, and
# it has no backup battery, or when your OS / disk itself is unstable.
#
# `sync_immediate` will greatly reduce the write throughput and put
# a lot more pressure on the disk. If you have lots of writes, it
# can pretty quickly kill your SSD for instance.
#HQL_SYNC_IMMEDIATE=false

# Sets the limit when the Raft will trigger the creation of a new
# state machine snapshot and purge all logs that are included in
# the snapshot.
# Higher values can achieve more throughput in very write heavy
# situations but will end up in more disk usage and longer
# snapshot creations / log purges.
# default: 10000
HQL_LOGS_UNTIL_SNAPSHOT=10000

# If given, these keys / certificates will be used to establish
# TLS connections between nodes.
#HQL_TLS_RAFT_KEY=tls/key.pem
#HQL_TLS_RAFT_CERT=tls/cert-chain.pem
#HQL_TLS_RAFT_DANGER_TLS_NO_VERIFY=true

#HQL_TLS_API_KEY=tls/key.pem
#HQL_TLS_API_CERT=tls/cert-chain.pem
#HQL_TLS_API_DANGER_TLS_NO_VERIFY=true

# Secrets for Raft internal authentication as well as for the API.
# These must be at least 16 characters long and you should provide
# different ones for both variables.
HQL_SECRET_RAFT={secret_raft}
HQL_SECRET_API={secret_api}

# You can either parse `ENC_KEYS` and `ENC_KEY_ACTIVE` from the
# environment with setting this value to `env`, or parse them from
# a file on disk with `file:path/to/enc/keys/file`
# default: env
#HQL_ENC_KEYS_FROM=env

# When the auto-backup task should run.
# Accepts cron syntax:
# "sec min hour day_of_month month day_of_week year"
# default: "0 30 2 * * * *"
#HQL_BACKUP_CRON="0 30 2 * * * *"

# Backups older than the configured days will be cleaned up on S3
# after the backup cron job `HQL_BACKUP_CRON`.
# default: 30
HQL_BACKUP_KEEP_DAYS=30

# Backups older than the configured days will be cleaned up locally
# after each `Client::backup()` and the cron job `HQL_BACKUP_CRON`.
# default: 3
HQL_BACKUP_KEEP_DAYS_LOCAL=3

# Access values for the S3 bucket where backups will be pushed to.
#HQL_S3_URL=https://s3.example.com
#HQL_S3_BUCKET=my_bucket
#HQL_S3_REGION=example
#HQL_S3_KEY=s3_key
#HQL_S3_SECRET=s3_secret

# You need to define at least one valid encryption key.
# These keys are used to encrypt the database backups that will
# be pushed to S3 storage.
#
# The format must match:
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
ENC_KEYS="
{enc_keys_trimmed}
"

# This identifies the key ID from the `ENC_KEYS` list, that
# should actively be used for new encryptions.
ENC_KEY_ACTIVE={enc_key_active}

# The password for the dashboard as b64 encoded Argon2ID hash
HQL_PASSWORD_DASHBOARD={password_dashboard_b64}

# Can be set to `true` during local dev and testing to issue
# insecure cookies
# default: false
HQL_INSECURE_COOKIE={insecure_cookie}
"#,
    ))
}
