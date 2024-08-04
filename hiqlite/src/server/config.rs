use crate::server::args::{ArgsConfig, ArgsGenerate};
use crate::server::password;
use cryptr::{utils, EncKeys};
use hiqlite::{Error, NodeConfig};
use tokio::fs;

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

    let path_file = default_config_file_path();
    if fs::File::open(&path_file).await.is_ok() {
        return Err(Error::Error(
            format!("Config file {} exists already", path_file).into(),
        ));
    }

    let password_dashboard = if let Some(password) = args.password {
        password::hash_password_b64(password).await?
    } else {
        let plain = utils::secure_random_alnum(16);
        println!("New password for the dashboard: {}", plain);
        password::hash_password_b64(plain).await?
    };
    let default_config = default_config(&password_dashboard, args.insecure_cookie)?;
    fs::write(&path_file, default_config).await?;
    println!("New default config file created: {}", path_file);

    #[cfg(target_family = "unix")]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path_file, Permissions::from_mode(0o600)).await?;
    }

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
# 2 nodes must be separated by 2 `\n`
#HQL_NODES="
#1 localhost:8100 localhost:8200
#2 localhost:8100 localhost:8200
#3 localhost:8100 localhost:8200
#"
HQL_NODES="
1 localhost:8100 localhost:8200
"

# The data dir hiqlite will store raft logs and state machine data in.
# default: hiqlite
HQL_DATA_DIR={}

# The file name of the SQLite database in the state machine folder.
# default: hiqlite.db
#HQL_FILENAME_DB=hiqlite.db

# If set to `true`, all SQL statements will be logged for debugging
# purposes.
# default: false
#HQL_LOG_STATEMENTS=false

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
HQL_SECRET_RAFT={}
HQL_SECRET_API={}

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

# Backups older than the configured days will be cleaned up after
# the backup cron job.
# default: 30
#HQL_BACKUP_KEEP_DAYS=30

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
{}
"

# This identifies the key ID from the `ENC_KEYS` list, that
# should actively be used for new encryptions.
ENC_KEY_ACTIVE={}

# The password for the dashboard as Argon2ID hash
HQL_PASSWORD_DASHBOARD={}

# Can be set to `true` during local dev and testing to issue
# insecure cookies
# default: false
HQL_INSECURE_COOKIE={}
"#,
        data_dir,
        secret_raft,
        secret_api,
        enc_keys_b64.trim(),
        enc_key_active,
        password_dashboard_b64,
        insecure_cookie,
    ))
}
