use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about)]
pub enum Args {
    /// Start a Hiqlite server
    Serve(ArgsConfig),

    /// Start a Hiqlite proxy, which acts as a single connection point for a remote client and
    /// load-balances requests to all cluster nodes.
    Proxy(ArgsProxy),

    /// Generate a new default config with safe values for testing
    GenerateConfig(ArgsGenerate),
}

#[derive(Debug, Clone, Parser)]
pub struct ArgsConfig {
    /// If you provide the node_id here, it will overwrite the value from the config file
    #[clap(long)]
    pub node_id: Option<u64>,

    /// The optional config file name to parse
    #[clap(short, long, default_value = "$HOME/.hiqlite/config")]
    pub config_file: String,

    /// Enable SQL statement logging
    #[clap(long)]
    pub log_statements: Option<bool>,

    /// Server Log Level
    #[clap(short, long, default_value = "info")]
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Parser)]
pub struct ArgsProxy {
    /// The optional config file name to parse
    #[clap(short, long, default_value = "$HOME/.hiqlite/config")]
    pub config_file: String,

    /// Server Log Level
    #[clap(short, long, default_value = "info")]
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Parser)]
pub struct ArgsGenerate {
    /// Set the password for the dashboard
    #[clap(short, long)]
    pub password: Option<String>,

    /// Issue insecure authn cookies for the dashboard. Do NOT use in production!
    #[clap(long, default_value = "false")]
    pub insecure_cookie: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}
