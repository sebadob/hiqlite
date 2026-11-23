//! The compile-time SQL check of a crate that has no `migrations` directory: the check reports
//! itself once as a warning and hands every query through untouched, which is what keeps a build
//! from ever breaking over a schema the macro cannot see.

use hiqlite::SqlShape;
use hiqlite::macros::sql;
use hiqlite::macros::sql_batch;
use hiqlite::macros::sql_map;
use hiqlite::sql_shape;
use std::borrow::Cow;

/// The mapped type of the `sql_map!` calls below, with its shape written by hand: a type that is
/// not derived with `FromRow` publishes its column names exactly this way.
#[derive(Debug)]
struct Point {
    x: i64,
    y: i64,
}

impl SqlShape for Point {
    const SHAPE: u64 = hiqlite::sql_shape(&["x", "y"]);
}

/// No schema is compiled for this crate, so nothing about this query is checkable here.
const QUERY: &str = sql!("SELECT id, ts FROM test WHERE ts > ?1");

#[test]
fn a_query_no_schema_can_check_is_the_string_itself() {
    assert_eq!(
        QUERY, "SELECT id, ts FROM test WHERE ts > ?1",
        "the macro must hand its literal through untouched"
    );
}

#[test]
fn a_batch_no_schema_can_check_is_the_string_itself() {
    const BATCH: &str = sql_batch!(
        "CREATE TABLE t (a INTEGER);
         INSERT INTO t VALUES (1);"
    );
    assert_eq!(
        BATCH,
        "CREATE TABLE t (a INTEGER);
         INSERT INTO t VALUES (1);",
        "a multi-statement string must keep its statements and its whitespace"
    );
}

#[test]
fn a_shape_no_schema_can_check_is_the_string_itself() {
    // The columns of this query are not the fields of `Point`, and no schema means no columns are
    // known, so the call is the string alone - the check of a shape needs a schema.
    const MAPPED: &str = sql_map!(Point, "SELECT id, ts FROM test WHERE ts > ?1");
    assert_eq!(MAPPED, "SELECT id, ts FROM test WHERE ts > ?1");

    let point = Point { x: 7, y: 9 };
    assert_eq!(point.x + point.y, 16, "{point:?} is read from two columns");
    assert_eq!(
        Point::SHAPE,
        sql_shape(&["y", "x"]),
        "the shape names exactly the columns {point:?} reads"
    );
}

#[test]
fn a_checked_query_is_cheap_to_use_as_a_query() {
    // `Into<Cow<'static, str>>` is what every query method takes, and a `&'static str` implements
    // it, so the value needs no deref, no clone, and can be used more than once.
    fn takes_a_query(sql: impl Into<Cow<'static, str>>) -> Cow<'static, str> {
        sql.into()
    }
    assert_eq!(takes_a_query(QUERY).as_ref(), QUERY);
    assert_eq!(takes_a_query(QUERY).as_ref(), QUERY);
}
