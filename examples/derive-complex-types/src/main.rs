#![allow(dead_code)]

use hiqlite::macros::embed::Embed;
use hiqlite::macros::{FromRow, SqlShape, TryFromRow, embed::*, params, sql, sql_batch, sql_map};
use hiqlite::{Error, NodeConfig, VecText};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use tokio::fs;
use tracing_subscriber::EnvFilter;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

/// This is our complex database entity.
/// The table definition can be found in `migrations/1_init.sql`.
///
/// The `FromRow` derive macro will create (in this example) the following impl:
///
/// ```rust, notest
/// impl ::std::convert::From<&mut ::hiqlite::Row<'_>> for Entity {
///     #[inline]
///     fn from(row: &mut ::hiqlite::Row) -> Self {
///         use ::std::str::FromStr;
///         Self {
///             id: row.get("id"),
///             name: row.get("name_db"),
///             desc: row.get("desc"),
///             vec_wrap: row.get("vec_wrap"),
///             sub: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
///             skipped: ::std::default::Default::default(),
///             some_int: row.get("some_int"),
///             my_enum: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
///             geo: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
///             left_right: row.get::<String>("left_right").parse().unwrap(),
///             up_down: row.get::<String>("ud").into(),
///             number: row.get::<i64>("num").into(),
///         }
///     }
/// }
/// ```
///
/// You have the following `column` attributes available:
///
/// - `rename = "some_col_name"` will rename the struct value to a different column name
/// - `skip` will skip that value in the `From<_>` impl and use `Default::default()`
/// - `flatten` can be used for any type that cannot be directly converted. You will use
///   this for all `struct`s, enums, or whatever other custom types you may have.
/// - `parse` if the `impl FromStr` for the type should be used
/// - `from_string` if the `impl From<String>` for the type should be used
/// - `from_i64` if the `impl From<i64>` for the type should be used
/// - `from_i32` if the `impl From<i32>` for the type should be used
///
/// You can combine `rename` with one of the `from_*` attributes, but not with `skip` or `flatten`.
/// The order does not matter in this case.
///
/// # The fast path, and the one rule it has
///
/// `query_map()` and `query_try_map()` resolve the field names of a mapped type into column indices
/// once per query and map every row by index, with no name lookup left in the row loop. The derive
/// registers a type for that path from the body it generates. A type that flattens a field is never
/// registered and gets no index mapping at all, and it is mapped by exactly the `row.get("name")`
/// reads you see in the impl above: a flattened part only has to provide a `From<&mut Row>`,
/// and the derive cannot assume anything more about it. Nothing about your queries changes with
/// that, it is only the mapping strategy that differs.
///
/// # The shape `sql_map!()` checks a query against
///
/// `sql_map!()` compares the columns a query returns against the `SHAPE` of the type it maps to, at
/// compile time. A derive publishes the column names of the fields it sees, and a `flatten` field
/// contributes the shape of its own type - so every part of a flattening struct has to publish its
/// names. A part derives them with `FromRow`, with the `SqlShape` derive next to a hand-written
/// mapping (`Geo` below), or with a hand-written `SqlShape` impl (`MyEnum` below).  A part that
/// publishes nothing is an error at its parent, naming that part: the columns a hand-written
/// `From<&mut Row>` reads are the one thing no derive can find out.
#[derive(Debug, FromRow)]
struct Entity {
    id: i64,
    #[column(rename = "name_db")]
    name: String,
    desc: Option<String>,
    /// This is using a smart wrapper type. This is `TEXT` column-backed and can be used to easily
    /// convert into a `Vec<_>` later on, since SQLite does not support arrays natively.
    /// In this example, `\n` will be used to separate the values, but it is free to choose.
    /// Use whatever `char` cannot create a conflict with the data you want to save.
    ///
    /// This type comes already with impls for converting it into a `hiqlite::Param` and to get it
    /// back from a DB column. See the code below for examples.
    vec_wrap: VecText<'\n'>,
    /// `flatten` will use the `FromRow` impl for `EntitySub` directly. It will have access to the
    /// exact same `Row`.
    #[column(flatten)]
    sub: EntitySub,
    #[column(skip)]
    skipped: Option<String>,
    /// Note: Because we enabled the `cast_ints` feature for `hiqlite`, we can use an
    /// `i32` here. Even though such a cast is not safe by definition, because SQLite stores all
    /// integers as `i64`, we can do the automatic cast, when we are sure that only this Rust
    /// program controls the input to the DB. In this case, Rust will give us the safety, even
    /// though SQLite is able to store bigger integers than an `i32`.
    some_int: i32,
    #[column(flatten)]
    my_enum: MyEnum,
    /// Our fully hand-written type from below, flattened like any derived one.
    #[column(flatten)]
    geo: Geo,
    /// If you already have an `impl FromStr` for your type, you can use it for the conversion like
    /// so, and you won't need an additional `flatten` + `From<&mut Row<_>>` impl.
    #[column(parse)]
    left_right: LeftRight,
    /// for `From<String>` impls
    #[column(rename = "ud", from_string)]
    up_down: UpDown,
    /// for `From<i64>` impls
    /// The order when using `rename` additionally does not matter.
    #[column(from_i64, rename = "num")]
    number: Number,
}

#[derive(Debug, FromRow)]
struct EntitySub {
    #[column(rename = "sub_id")]
    id: i64,
    #[column(rename = "sub_name")]
    name: String,
    #[column(flatten)]
    sub_sub: EntitySubSub,
    url: Option<url::Url>,
    /// UUIDs can be stored as `BLOB` and parsed directly
    uuid: Option<uuid::Uuid>,
}

#[derive(Debug, FromRow)]
struct EntitySubSub {
    secret: String,
}

/// The non-panicking family on a nested type: `TryFromRow` reads every column as a `Result`, and it
/// flattens a part that derives `FromRow` - which emits the `TryFrom` side of that part too.
#[derive(Debug, TryFromRow, SqlShape)]
struct SecretRow {
    #[column(flatten)]
    inner: EntitySubSub,
}

#[derive(Debug)]
enum LeftRight {
    Left,
    Right,
}

impl FromStr for LeftRight {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let slf = match s {
            "left" => Self::Left,
            "right" => Self::Right,
            _ => {
                return Err(Error::Error("parse err".into()));
            }
        };
        Ok(slf)
    }
}

impl LeftRight {
    fn as_str(&self) -> &str {
        match self {
            LeftRight::Left => "left",
            LeftRight::Right => "right",
        }
    }
}

#[derive(Debug)]
enum UpDown {
    Up,
    Down,
}

impl From<String> for UpDown {
    fn from(value: String) -> Self {
        match value.as_str() {
            "up" => Self::Up,
            "down" => Self::Down,
            _ => panic!("corrupted database"),
        }
    }
}

impl UpDown {
    fn as_str(&self) -> &str {
        match self {
            UpDown::Up => "up",
            UpDown::Down => "down",
        }
    }
}

#[derive(Debug)]
enum Number {
    One = 1,
    Two,
    Three,
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        match value {
            1 => Self::One,
            2 => Self::Two,
            3 => Self::Three,
            _ => panic!("corrupted database"),
        }
    }
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
/// `From<&mut hiqlite::Row<'_>>` in combination with `#[column(flatten)]` on the parent. This
/// gives you the most amount of flexibility, and you only need to remember this single trait, not
/// like 5-8 different ones.
///
/// A custom body reads the columns it wants and nobody can read them out of the type, so the shape
/// for `sql_map!()` is the one thing you write yourself: an `enum` is not a struct, so the
/// `SqlShape` derive is not for it, and this is the whole of what you add.
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

impl hiqlite::SqlShape for MyEnum {
    const SHAPE: u64 = hiqlite::sql_shape(&["enum_value"]);
}

/// Two `INTEGER` columns, read by a hand-written body that narrows them to `i32` - the same thing
/// the `#[column(from_i32)]` attribute does on a single field, done on two at once and with
/// our own message.
///
/// `#[derive(SqlShape)]` is the shape-only counterpart of `FromRow`: same fields, same
/// `#[column(...)]` attributes, same validation, and it emits only the names a `sql_map!()` of
/// `Entity` checks against.
/// The mapping below stays ours.
#[derive(Debug, SqlShape)]
struct Geo {
    lat: i32,
    lon: i32,
}

impl From<&mut hiqlite::Row<'_>> for Geo {
    fn from(row: &mut hiqlite::Row<'_>) -> Self {
        let mut get = |name: &str| {
            i32::try_from(row.get::<i64>(name))
                .unwrap_or_else(|_| panic!("column '{name}' does not fit into an i32"))
        };
        Self {
            lat: get("lat"),
            lon: get("lon"),
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

    let config = NodeConfig::from_toml("../../hiqlite.toml", None, None, None).await?;
    let client = hiqlite::start_node(config).await?;

    log("Apply our database migrations");
    client.migrate::<Migrations>().await?;

    log("Insert a row");

    let vec_wrap: VecText<'\n'> = VecText::new(&["Entry 1", "Entry 2", "And another one"])?;
    let url: url::Url = "http://localhost:8080".parse().unwrap();
    let uuid = uuid::Uuid::now_v7();

    // `sql!()` prepares this statement against your `migrations` directory at compile time, so a
    // typo in a table or column name is a compile error and not a runtime one. It emits the literal
    // untouched, so nothing at a call site changes.
    client
        .execute(
            sql!(
                r#"
INSERT INTO complex (
    id, name_db, desc, vec_wrap, some_int, left_right, ud, num, sub_id, sub_name, url, uuid,
    secret, enum_value, lat, lon
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
"#
            ),
            params!(
                13,
                "Base Name",
                "Some Description",
                &vec_wrap,
                27,
                LeftRight::Left.as_str(),
                UpDown::Up.as_str(),
                Number::Two as i32,
                1337,
                "Sub Name",
                &url,
                &uuid,
                "IAmSoSecureYouWillNeverGuess",
                "Some 'Other' value for MyEnum",
                52_520,
                13_400,
            ),
        )
        .await?;

    log("Let's get the data back from the DB");

    // `sql_map!()` checks the columns this query returns against the shape of `Entity` at compile
    // time: its own fields, plus `EntitySub`, `EntitySubSub`, `MyEnum` and `Geo` through the shapes
    // those publish.  `SELECT *` is enough for SQLite to know every column, so a column that no
    // field of this tree reads, and a field no column of this query returns, both fail the build.
    let res: Entity = client
        .query_map_one(
            sql_map!(Entity, "SELECT * FROM complex WHERE id = $1"),
            params!(13),
        )
        .await?;

    debug(&res);

    assert_eq!(res.id, 13);
    assert_eq!(res.name, "Base Name");
    assert_eq!(res.desc.as_deref(), Some("Some Description"));
    assert_eq!(res.vec_wrap, vec_wrap);
    assert_eq!(res.some_int, 27);
    assert_eq!(res.left_right as i32, LeftRight::Left as i32);
    assert_eq!(res.up_down as i32, UpDown::Up as i32);
    assert_eq!(res.number as i32, Number::Two as i32);
    assert_eq!(res.geo.lat, 52_520);
    assert_eq!(res.geo.lon, 13_400);

    assert_eq!(res.sub.id, 1337);
    assert_eq!(res.sub.name, "Sub Name");
    assert_eq!(res.sub.url, Some(url));
    assert_eq!(res.sub.uuid, Some(uuid));

    assert_eq!(res.sub.sub_sub.secret, "IAmSoSecureYouWillNeverGuess");
    let MyEnum::Other(s) = res.my_enum else {
        unreachable!()
    };
    assert_eq!(s, "Some 'Other' value for MyEnum");

    // this is how we can convert the `VecText` into a real `Vec<_>`.
    let vs = res.vec_wrap.parse::<String>()?;
    log(format!(
        "We can convert our smart Wrapper `VecText` easily into a `Vec<String>` \
        for instance: {vs:?}"
    ));

    // You can also convert to int `Vec`s and many other types like so:
    let v = VecText::<','>::new(&[1, 2, -3])?;
    let r = v.parse::<i32>()?;
    assert_eq!(&[1, 2, -3], r.as_slice());

    log("The non-panicking family, on a type that flattens a derived part");

    // `TryFromRow` returns an `hiqlite::Error` for a missing column or a value that does not fit,
    // where the panicking family above panics, and `query_try_map_one` hands that `Result` to
    // you as the value of the row.  A `sql_map!()` checks it exactly like the other one: a type
    // that does not derive `FromRow` publishes its shape through `#[derive(SqlShape)]`.
    let secret: SecretRow = client
        .query_try_map_one(
            sql_map!(SecretRow, "SELECT secret FROM complex WHERE id = $1"),
            params!(13),
        )
        .await??;
    assert_eq!(secret.inner.secret, "IAmSoSecureYouWillNeverGuess");

    log("A multi-statement string for `batch()`, checked the same way");

    // `sql_batch!()` is the multi-statement shape of the same check, for `Client::batch` and
    // `Client::txn`, where the statements go into one string.
    client
        .batch(sql_batch!(
            "UPDATE complex SET desc = 'Updated Description' WHERE id = 13;
             UPDATE complex SET some_int = 28 WHERE id = 13;"
        ))
        .await?;

    let updated: Updated = client
        .query_map_one(
            sql_map!(Updated, "SELECT desc, some_int FROM complex WHERE id = $1"),
            params!(13),
        )
        .await?;
    assert_eq!(updated.desc.as_deref(), Some("Updated Description"));
    assert_eq!(updated.some_int, 28);

    log("That's it - our complex Entity mapped successfully");

    Ok(())
}

/// A second mapped type over the same table: a `sql_map!()` checks each query against the type it
/// names, so a partial read is checked as exactly that partial read.
#[derive(Debug, FromRow)]
struct Updated {
    desc: Option<String>,
    some_int: i32,
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}

fn debug<S: Debug>(s: &S) {
    println!("\n\n>>> {s:?}\n",);
}
