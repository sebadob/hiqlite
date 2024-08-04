// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> Result<(), hiqlite::Error> {
    server::server().await?;

    Ok(())
}

#[cfg(not(feature = "server"))]
fn main() {
    panic!("If you want to compile as binary, you need to enable the 'server' feature");
}
