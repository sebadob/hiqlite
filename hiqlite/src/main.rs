// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

#[cfg(feature = "server")]
mod server;

#[tokio::main]
async fn main() -> Result<(), hiqlite::Error> {
    #[cfg(feature = "server")]
    server::server().await?;

    #[cfg(not(feature = "server"))]
    panic!("If you want to compile as binary, you need to enable the 'server' feature");

    Ok(())
}
