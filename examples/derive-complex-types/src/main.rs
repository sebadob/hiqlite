use hiqlite::{Error, NodeConfig};
use hiqlite_macros::embed::*;
use hiqlite_macros::{params, FromRow};
use std::fmt::{Debug, Display};
use tokio::fs;
use tracing_subscriber::EnvFilter;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

/// This is our complex database entity.
/// The table definition can be found in `migrations/1_init.sql`.
///
/// The `FromRow` derive macro will create the following impl:
///
/// ```rust, notest
///impl ::std::convert::From<&mut ::hiqlite::Row<'_>> for Entity {
///    #[inline]
///    fn from(row: &mut ::hiqlite::Row) -> Self {
///        Self {
///            id: row.get("id"),
///            name: row.get("name_db"),
///            desc: row.get("desc"),
///            sub: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
///            skipped: ::std::default::Default::default(),
///            some_int: row.get("some_int"),
///            my_enum: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
///        }
///    }
///}
/// ```
///
/// You have the following `column` attributes available:
/// - `column(rename = "name_db")` will rename the struct value to a different column name
/// - `column(skip)` will skip that value in the `From<_>` impl and use `Default::default()`
/// - `column(flatten)` can be used for any type that cannot be directly converted. You will use
///   this for all `struct`s, enums, or whatever other custom types you may have.
#[derive(Debug, FromRow)]
struct Entity {
    id: i64,
    #[column(rename = "name_db")]
    name: String,
    desc: Option<String>,
    #[column(flatten)]
    sub: EntitySub,
    #[column(skip)]
    skipped: Option<String>,
    some_int: i64,
    #[column(flatten)]
    my_enum: MyEnum,
}

#[derive(Debug, FromRow)]
struct EntitySub {
    #[column(rename = "sub_id")]
    id: i64,
    #[column(rename = "sub_name")]
    name: String,
    #[column(flatten)]
    sub_sub: EntitySubSub,
}

#[derive(Debug, FromRow)]
struct EntitySubSub {
    secret: String,
}

#[derive(Debug)]
enum MyEnum {
    Empty,
    One,
    Two,
    Other(String),
}

/// This shows a fully custom implementation to get the proper value from the database.
///
/// In contrast to many other crates, `hiqlite`s goal is to keep it simple here. You will not
/// have multiple different `From*` traits you need to impl, and even worse, different ones for
/// different Rust types. No matter what it is, you will always need to impl just
/// `From<&mut hiqlite::Row<'_>>` in combination with `#[column(flatten)]` in the parent. This
/// gives you the most amount of flexibility, and you only need to remember this single trait, not
/// 5-8 different ones.
impl From<&mut hiqlite::Row<'_>> for MyEnum {
    fn from(row: &mut hiqlite::Row<'_>) -> Self {
        if let Some(value) = row.get::<Option<String>>("enum_value") {
            match value.as_str() {
                "One" => MyEnum::One,
                "Two" => MyEnum::Two,
                _ => MyEnum::Other(value),
            }
        } else {
            Self::Empty
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // make sure we always start clean
    let _ = fs::remove_dir_all("./data").await;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("info"))
        .init();

    let config = NodeConfig::from_toml("../../hiqlite.toml", None, None).await?;
    let client = hiqlite::start_node(config).await?;

    log("Apply our database migrations");
    client.migrate::<Migrations>().await?;

    log("Insert a row");
    client
        .execute(
            r#"
INSERT INTO complex (id, name_db, desc, some_int, sub_id, sub_name, secret, enum_value)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
"#,
            params!(
                13,
                "Base Name",
                "Some Description",
                27,
                1337,
                "Sub Name",
                "IAmSoSecureYouWillNeverGuess",
                "Some 'Other' value for MyEnum"
            ),
        )
        .await?;

    log("Let's get the data back from the DB");

    let res: Entity = client
        .query_map_one("SELECT * FROM complex WHERE id = $1", params!(13))
        .await?;

    debug(&res);

    log("That's it - our complex Entity mapped successfully");

    Ok(())
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}

fn debug<S: Debug>(s: &S) {
    println!("\n\n>>> {s:?}\n",);
}
