// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> Result<(), hiqlite::Error> {
    hiqlite::server::server().await?;
    Ok(())
}

#[cfg(not(feature = "server"))]
fn main() {
    panic!("to compile as binary, enable the 'server' feature");
}
