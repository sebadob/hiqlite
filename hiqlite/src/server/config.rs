use crate::helpers::{read_line_stdin, set_path_access};
use crate::server::args::{ArgsConfig, ArgsGenerate};
use crate::server::password;
use crate::{Error, NodeConfig, REFERENCE_CONFIG};
use cryptr::{EncKeys, utils};
use std::fmt::Write;
use tokio::fs;

static REFERENCE_PROXY_CONFIG: &str = include_str!("../../../REFERENCE_CONFIG_PROXY.toml");

pub async fn build_node_config(args: ArgsConfig) -> Result<NodeConfig, Error> {
    let config_path = if args.config_file == "$HOME/.hiqlite/hiqlite.toml" {
        default_config_file_path()
    } else {
        args.config_file
    };
    let mut config = NodeConfig::from_toml(&config_path, None, None, None).await?;

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
        // Make sure the existing file is not world-readable while we rewrite it.
        set_path_access(&path_file, 0o600).await?;
    }

    let pwd_plain = if args.password {
        let plain;
        loop {
            println!("Provide a password with at least 16 characters: ");
            let line = read_line_stdin().await?;
            if line.chars().count() >= 16 {
                plain = line;
                break;
            } else {
                eprintln!(
                    "Input too short - has only {} characters",
                    line.chars().count()
                );
            }
        }
        plain
    } else {
        utils::secure_random_alnum(24)
    };
    println!("New password for the dashboard: {}", pwd_plain);
    let password_dashboard = password::hash_password_b64(pwd_plain).await?;

    let default_config = default_config(&password_dashboard, args.insecure_cookie)?;

    // Create the file with restrictive permissions up front so it is never
    // world-readable between creation and the chmod below. On unix we can set the
    // mode at creation time; elsewhere fall back to a plain write.
    #[cfg(target_family = "unix")]
    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path_file)
            .await?;
        file.write_all(default_config.as_bytes()).await?;
    }
    #[cfg(not(target_family = "unix"))]
    {
        fs::write(&path_file, default_config).await?;
    }

    println!("New default proxy config file created: {}", path_file);

    Ok(())
}

pub async fn generate_proxy() -> Result<(), Error> {
    let path = default_config_dir();
    fs::create_dir_all(&path).await?;
    set_path_access(&path, 0o700).await?;

    let path_file = default_proxy_config_file_path();
    if fs::File::open(&path_file).await.is_ok() {
        eprint!(
            "Proxy Config file {} exists already. Overwrite? (yes): ",
            path_file
        );
        let line = read_line_stdin().await?;
        if line != "yes" {
            return Ok(());
        }
        // Make sure the existing file is not world-readable while we rewrite it.
        set_path_access(&path_file, 0o600).await?;
    }

    // Create the file with restrictive permissions up front so it is never
    // world-readable between creation and the chmod below. On unix we can set the
    // mode at creation time; elsewhere fall back to a plain write.
    #[cfg(target_family = "unix")]
    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path_file)
            .await?;
        file.write_all(REFERENCE_PROXY_CONFIG.as_bytes()).await?;
    }
    #[cfg(not(target_family = "unix"))]
    {
        fs::write(&path_file, REFERENCE_PROXY_CONFIG).await?;
    }

    println!("New default proxy config file created: {}", path_file);

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

#[inline]
pub(crate) fn default_proxy_config_file_path() -> String {
    format!("{}/hiqlite-proxy.toml", default_config_dir())
}

fn default_config(password_dashboard_b64: &str, insecure_cookie: bool) -> Result<String, Error> {
    let data_dir = format!("{}/data", default_config_dir());
    let secret_raft = utils::secure_random_alnum(32);
    let secret_api = utils::secure_random_alnum(32);
    let enc_keys = EncKeys::generate()?;
    let enc_keys_b64 = enc_keys.keys_as_b64_vec();
    let enc_key = enc_keys_b64.first().unwrap();
    let enc_key_active = enc_keys.enc_key_active;

    let mut config = String::with_capacity(REFERENCE_CONFIG.len());

    for line in REFERENCE_CONFIG
        .lines()
        .skip_while(|l| !l.starts_with("[hiqlite]"))
    {
        // bool temp values will be invalid TOMLM, and it would make other tests fail
        let l = if insecure_cookie && line.starts_with("insecure_cookie = false") {
            "insecure_cookie = true".to_string()
        } else {
            line.replace("{{ DATA_DIR }}", &data_dir)
                .replace("{{ SECRET_RAFT }}", &secret_raft)
                .replace("{{ SECRET_API }}", &secret_api)
                .replace("{{ ENC_KEY }}", enc_key)
                .replace("{{ ENC_KEY_ACTIVE }}", &enc_key_active)
                .replace("{{ PASSWORD_DASHBOARD }}", password_dashboard_b64)
        };
        writeln!(config, "{l}")?;
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_server_config() {
        assert!(REFERENCE_CONFIG.contains("{{ DATA_DIR }}"));
        assert!(REFERENCE_CONFIG.contains("{{ SECRET_RAFT }}"));
        assert!(REFERENCE_CONFIG.contains("{{ SECRET_API }}"));
        assert!(REFERENCE_CONFIG.contains("{{ ENC_KEY }}"));
        assert!(REFERENCE_CONFIG.contains("{{ ENC_KEY_ACTIVE }}"));
        assert!(REFERENCE_CONFIG.contains("{{ PASSWORD_DASHBOARD }}"));

        let c = default_config("IDontCareAboutB64", true).unwrap();

        assert!(!c.contains("{{ DATA_DIR }}"));
        assert!(!c.contains("{{ SECRET_RAFT }}"));
        assert!(!c.contains("{{ SECRET_API }}"));
        assert!(!c.contains("{{ ENC_KEY }}"));
        assert!(!c.contains("{{ ENC_KEY_ACTIVE }}"));
        assert!(!c.contains("{{ PASSWORD_DASHBOARD }}"));
        assert!(c.contains("insecure_cookie = true"));
    }
}
