use crate::Error;
use bincode_next::{Decode, Encode};
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::types::FromSql;
use rusqlite::{Error as SqliteError, Row as SqliteRow, Statement};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::Arc;
use tracing::error;

/// A column name -> index map of a prepared statement, built once per query.
///
/// rusqlite resolves a column name with a linear scan that asks SQLite for the name of every
/// candidate column, so a `Row::get("name")` costs one C call per column of the result set, for
/// every single row. A struct with N fields over a result set with N columns therefore does
/// N * (N + 1) / 2 of those calls per row.  Building the map once per query turns every later
/// lookup into a hash probe over names we already fetched.
#[derive(Debug)]
pub struct ColIdx {
    map: FxHashMap<String, u16>,
}

impl ColIdx {
    pub(crate) fn from_stmt(stmt: &Statement) -> Self {
        let names = stmt.column_names();
        let mut map = FxHashMap::default();
        // The result set tells us its column count upfront, so the map is reserved once and never
        // has to grow while we fill it.
        map.reserve(names.len());
        // `entry().or_insert()` keeps the lowest index, so a query with duplicate column names,
        // like a join over two tables sharing one, maps to the first column exactly like
        // rusqlite's own scan does.
        for (idx, name) in names.enumerate() {
            map.entry(name.to_string()).or_insert(idx as u16);
        }
        Self { map }
    }

    /// The same index over the column names of a result set that was read into owned rows.
    pub(crate) fn from_names(columns: &[String]) -> Self {
        let mut map = FxHashMap::default();
        map.reserve(columns.len());
        for (idx, name) in columns.iter().enumerate() {
            map.entry(name.clone()).or_insert(idx as u16);
        }
        Self { map }
    }

    /// The index a column name resolves to, an exact match like the owned rows always did.
    fn index(&self, idx: &str) -> Option<usize> {
        self.map.get(idx).map(|i| usize::from(*i))
    }

    /// Reads a column of a borrowed row, resolving its name through the map.
    ///
    /// A name that is not in the map goes through rusqlite's own resolution, which matches
    /// column names case-insensitively, so that behaviour is preserved.
    fn borrowed_get<T: FromSql>(row: &SqliteRow, cols: &Self, idx: &str) -> Result<T, SqliteError> {
        match cols.map.get(idx) {
            Some(i) => row.get(*i as usize),
            None => row.get(idx),
        }
    }
}

#[derive(Debug)]
pub enum Row<'a> {
    /// A row borrowed from a local statement, with the column index of that statement.
    Borrowed(&'a SqliteRow<'a>, &'a ColIdx),
    /// A row that was mapped into owned values to travel through the network layer, together with
    /// the column index of the result set it is part of.
    Owned(RowOwned, Arc<ColIdx>),
}

impl Row<'_> {
    ///
    /// # Panics
    ///
    /// If the column `idx` is not on the row, or its value cannot be converted into `T`.
    pub fn get<T>(&mut self, idx: &str) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Row::Borrowed(b, cols) => {
                let res = ColIdx::borrowed_get(b, cols, idx);
                if res.is_err() {
                    error!("Cannot convert column index '{idx}' to requested type");
                }
                res.unwrap()
            }
            Row::Owned(o, cols) => o.try_get(cols.as_ref(), idx).unwrap_or_else(|err| {
                panic!("Cannot convert column index '{idx}' to requested type: {err:?}")
            }),
        }
    }

    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that is not on the row, or whose value cannot be converted
    /// into `T`; on a borrowed row that is the [`Error::Sqlite`] of `rusqlite`.
    pub fn try_get<T>(&mut self, idx: &str) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Self::Borrowed(r, cols) => ColIdx::borrowed_get(r, cols, idx).map_err(Error::from),
            Self::Owned(o, cols) => o.try_get(cols.as_ref(), idx),
        }
    }
}

/// The values of one row, in the column order of the result set it was read from.  The names of
/// those columns are not repeated per row; they are the `columns` of the [`RowsOwned`] this row
/// belongs to.
///
/// Reading a value takes it out of its slot, so a column can be read exactly once, like the
/// per-row name scan did before.  Slots stay in place, which is what keeps the shared column
/// index of the result set valid for every lookup of the row.
#[derive(Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct RowOwned {
    pub(crate) values: Vec<Option<ValueOwned>>,
}

/// A result set that travels through the network, or into the dashboard: the column names of the
/// query once, and the values of every row in that same order.
#[derive(Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct RowsOwned {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<RowOwned>,
}

/// [`RowsOwned`] for statements that report every row as its own `Result`, like
/// `execute_returning`, where one failing row sits next to the rows that succeeded.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct RowsOwnedChecked {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Result<RowOwned, Error>>,
}

impl RowsOwned {
    pub(crate) fn new(columns: Vec<String>, rows: Vec<RowOwned>) -> Self {
        Self { columns, rows }
    }

    /// Split into the shared column index and the rows, the shape [`Row::Owned`] is built from.
    pub(crate) fn into_parts(self) -> (Arc<ColIdx>, Vec<RowOwned>) {
        let cols = self.col_idx();
        (cols, self.rows)
    }

    fn col_idx(&self) -> Arc<ColIdx> {
        Arc::new(ColIdx::from_names(&self.columns))
    }
}

impl RowsOwnedChecked {
    pub(crate) fn new(columns: Vec<String>, rows: Vec<Result<RowOwned, Error>>) -> Self {
        Self { columns, rows }
    }

    /// Split into the shared column index and the rows, keeping each row's `Result`.
    pub(crate) fn into_parts(self) -> (Arc<ColIdx>, Vec<Result<RowOwned, Error>>) {
        let cols = self.col_idx();
        (cols, self.rows)
    }

    fn col_idx(&self) -> Arc<ColIdx> {
        Arc::new(ColIdx::from_names(&self.columns))
    }
}

impl RowOwned {
    #[inline]
    pub(crate) fn from_row_column(row: &rusqlite::Row, columns: &[ColumnInfo]) -> Self {
        let mut cols = Vec::with_capacity(columns.len());

        for (i, info) in columns.iter().enumerate() {
            let value = match info.typ {
                ColumnType::Expr => {
                    // returned expressions can be any type we don't know in advance
                    // The chain below covers every SQLite storage class, so the final Null is
                    // only reachable for a genuine SQL NULL.
                    if let Ok(text) = row.get::<_, String>(i) {
                        ValueOwned::Text(text)
                    } else if let Ok(i) = row.get::<_, i64>(i) {
                        ValueOwned::Integer(i)
                    } else if let Ok(r) = row.get::<_, f64>(i) {
                        ValueOwned::Real(r)
                    } else if let Ok(b) = row.get::<_, Vec<u8>>(i) {
                        ValueOwned::Blob(b)
                    } else {
                        ValueOwned::Null
                    }
                }
                // ColumnType::Expr => row.get(i).map(ValueOwned::Text).unwrap_or(ValueOwned::Null),
                // Typed columns: a genuine SQL NULL maps to Null; a stored value that cannot be
                // converted is a type mismatch and must fail loudly instead of silently
                // becoming Null.
                ColumnType::Integer => match row.get::<_, Option<i64>>(i) {
                    Ok(Some(v)) => ValueOwned::Integer(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Integer but the stored value \
                         cannot be converted to i64: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Real => match row.get::<_, Option<f64>>(i) {
                    Ok(Some(v)) => ValueOwned::Real(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Real but the stored value \
                         cannot be converted to f64: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Text => match row.get::<_, Option<String>>(i) {
                    Ok(Some(v)) => ValueOwned::Text(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Text but the stored value \
                         cannot be converted to String: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Blob => match row.get::<_, Option<Vec<u8>>>(i) {
                    Ok(Some(v)) => ValueOwned::Blob(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Blob but the stored value \
                         cannot be converted to Vec<u8>: {err:?}",
                        info.name
                    ),
                },
            };

            cols.push(Some(value));
        }

        Self { values: cols }
    }
}

impl RowOwned {
    pub(crate) fn try_get<T: TryFrom<ValueOwned, Error = crate::Error>>(
        &mut self,
        cols: &ColIdx,
        idx: &str,
    ) -> Result<T, Error> {
        let not_found = || Error::QueryParams(format!("column '{idx}' not found").into());
        match cols.index(idx).and_then(|i| self.values.get_mut(i)) {
            Some(slot) => match slot.take() {
                Some(value) => T::try_from(value),
                None => Err(not_found()),
            },
            None => Err(not_found()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub typ: ColumnType,
}

impl ColumnInfo {
    /// The mapping information of every column of a prepared statement, in result-set order.
    #[inline]
    pub(crate) fn mapping_cols_from_stmt<'a>(
        columns: impl ExactSizeIterator<Item = rusqlite::Column<'a>>,
    ) -> Vec<Self> {
        let mut cols = Vec::with_capacity(columns.len());
        for col in columns {
            cols.push(Self {
                name: col.name().to_string(),
                typ: ColumnType::from_decl_type(col.decl_type()),
            });
        }
        cols
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnType {
    Expr,
    Integer,
    Real,
    Text,
    Blob,
}

impl ColumnType {
    #[inline]
    fn from_decl_type(typ: Option<&str>) -> Self {
        // we will always match as few character as possible for improved speed
        if let Some(t) = typ {
            // We will check "correct" types first for better speed with correct definitions.
            // SQLITE will convert int, integer, text, blob, real to UPPERCASE on creation.
            if t.starts_with("INT") || t.starts_with("int") {
                return Self::Integer;
            }

            match t {
                "TEXT" | "text" => Self::Text,
                "BLOB" | "blob" => Self::Blob,
                "REAL" | "real" => Self::Real,
                _ => {
                    // When the proper types don't exist, try check type-affinity.
                    // Type affinity matches will NOT be converted to uppercase automatically!

                    if t.is_empty() {
                        return Self::Blob;
                    }

                    let ty = t.to_uppercase();

                    // 3.1. Determination Of Column Affinity from SQLite docs:
                    // https://www.sqlite.org/datatype3.html

                    // .starts_with("INT") already checked
                    // INT
                    // INTEGER
                    // INT2
                    // INT8
                    //
                    // TINYINT
                    // SMALLINT
                    // MEDIUMINT
                    // BIGINT
                    // UNSIGNED BIG INT
                    if ty.contains("INT") {
                        Self::Integer

                    // "TEXT already checked
                    // TEXT
                    //
                    // CHARACTER(20)
                    // VARCHAR(255)
                    // VARYING CHARACTER(255)
                    // NCHAR(55)
                    // NATIVE CHARACTER(70)
                    // NVARCHAR(100)
                    // CLOB
                    } else if ty.contains("CHAR") || ty.contains("CLOB") {
                        Self::Text

                    // .starts_with("RE") already checked
                    // REAL
                    //
                    // DOUBLE
                    // DOUBLE PRECISION
                    // FLOAT
                    } else if ty.contains("FLOA") || ty.contains("DOUB") {
                        Self::Real

                    // Anything SQLite cannot match properly should be an INTEGER / NUMERIC.
                    } else {
                        Self::Integer
                    }
                }
            }
        } else {
            Self::Expr
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ValueOwned {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl ValueOwned {
    #[inline]
    fn try_as_str(&self) -> Result<&str, Error> {
        match self {
            ValueOwned::Text(s) => Ok(s.as_str()),
            _ => Err(Error::Sqlite("Cannot convert ValueOwned to &str".into())),
        }
    }
}

impl TryFrom<ValueOwned> for i64 {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i),
            _ => Err(Error::Sqlite("Cannot convert into i64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<i64> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => i64::try_from(v).map(Some),
        }
    }
}

#[cfg(any(feature = "cast_ints", feature = "cast_ints_unchecked"))]
#[allow(clippy::cast_sign_loss)]
// Both `cast_ints` features exist to downcast the DB's `i64` into a smaller integer on purpose:
// the unchecked one wraps exactly like `as`, the checked one clamps to the target's bounds.
// Clippy cannot see that clamp, so the sign loss it warns about is exactly what these features
// ask for. The allow sits on the module because the casts come out of a macro, where an allow on
// the generated function does not reach them.
mod cast_ints {
    impl TryFrom<super::ValueOwned> for u64 {
        type Error = crate::Error;

        #[inline]
        fn try_from(value: super::ValueOwned) -> Result<Self, Self::Error> {
            let i = i64::try_from(value)?;
            // `cast_ints` clamps: anything below zero lands on 0.
            Ok(i.max(0).cast_unsigned())
        }
    }

    impl TryFrom<super::ValueOwned> for Option<u64> {
        type Error = crate::Error;

        #[inline]
        fn try_from(value: super::ValueOwned) -> Result<Option<u64>, Self::Error> {
            let i: Option<i64> = value.try_into()?;
            Ok(i.map(|i| i.cast_unsigned()))
        }
    }

    macro_rules! downcast_int(
        ($t:ty) => (
            impl TryFrom<super::ValueOwned> for $t {
                type Error = crate::Error;

                #[inline]
                fn try_from(value: super::ValueOwned) -> Result<Self, Self::Error> {
                    let i = i64::try_from(value)?;

                    #[cfg(feature = "cast_ints_unchecked")]
                    return Ok(i as $t);

                    #[cfg(feature = "cast_ints")]
                    if i < 0 {
                        Ok(::core::cmp::max(<$t>::MIN as i64, i) as $t)
                    } else {
                        Ok(::core::cmp::min(<$t>::MAX as i64, i) as $t)
                    }
                }
            }
        )
    );

    downcast_int!(i32);
    downcast_int!(i16);
    downcast_int!(i8);

    downcast_int!(u32);
    downcast_int!(u16);
    downcast_int!(u8);

    macro_rules! downcast_int_opt(
        ($t:ty) => (
            impl TryFrom<super::ValueOwned> for Option<$t> {
                type Error = crate::Error;

                #[inline]
                fn try_from(value: super::ValueOwned) -> Result<Option<$t>, Self::Error> {
                    let i: Option<i64> = value.try_into()?;
                    Ok(i.map(|i| i as $t))
                }
            }
        )
    );

    downcast_int_opt!(i32);
    downcast_int_opt!(i16);
    downcast_int_opt!(i8);

    downcast_int_opt!(u32);
    downcast_int_opt!(u16);
    downcast_int_opt!(u8);
}

impl TryFrom<ValueOwned> for f64 {
    type Error = Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Real(r) => Ok(r),
            _ => Err(Error::Sqlite("Cannot convert into f64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<f64> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => f64::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for bool {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i == 1),
            // SQLite stores a boolean as an exact 1.0, and only that: a margin of error would
            // make near-values true, which is exactly what a stored flag must never do.
            ValueOwned::Real(i) => Ok(i.total_cmp(&1.0) == Ordering::Equal),
            ValueOwned::Text(s) => Ok(s.as_str() == "true"),
            _ => Err(Error::Sqlite("Cannot convert into bool".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<bool> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => v.try_into().map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for String {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => Ok(s),
            _ => Err(Error::Sqlite("Cannot convert into String".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<String> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => String::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for Vec<u8> {
    type Error = Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => Ok(b),
            _ => Err(Error::Sqlite("Cannot convert into Vec<u8>".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<Vec<u8>> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => Vec::try_from(v).map(Some),
        }
    }
}

impl<const N: usize> TryFrom<ValueOwned> for [u8; N] {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => Ok(b
                .try_into()
                .map_err(|_| Error::Sqlite("Cannot convert into [u8]: incorrect length".into()))?),
            _ => Err(Error::Sqlite("Cannot convert into [u8]".into())),
        }
    }
}

impl<const N: usize> TryFrom<ValueOwned> for Option<[u8; N]> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => <[u8; N]>::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveDate {
    type Error = crate::Error;

    /// "YYYY-MM-DD" => ISO 8601 calendar date without timezone.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        match Self::parse_from_str(s, "%F") {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveTime {
    type Error = crate::Error;

    /// "HH:MM"/"HH:MM:SS"/"HH:MM:SS.SSS" => ISO 8601 time without timezone.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        let fmt = match s.len() {
            5 => "%H:%M",
            8 => "%T",
            _ => "%T%.f",
        };

        match Self::parse_from_str(s, fmt) {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveDateTime {
    type Error = crate::Error;

    /// "YYYY-MM-DD HH:MM:SS"/"YYYY-MM-DD HH:MM:SS.SSS" => ISO 8601 combined date
    /// and time without timezone. ("YYYY-MM-DDTHH:MM:SS"/"YYYY-MM-DDTHH:MM:SS.SSS"
    /// also supported)
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        let fmt = if s.len() >= 11 && s.as_bytes()[10] == b'T' {
            "%FT%T%.f"
        } else {
            "%F %T%.f"
        };

        match Self::parse_from_str(s, fmt) {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for DateTime<Utc> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Utc>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        {
            // Try to parse value as rfc3339 first.
            let s = value.try_as_str()?;

            let fmt = if s.len() >= 11 && s.as_bytes()[10] == b'T' {
                "%FT%T%.f%#z"
            } else {
                "%F %T%.f%#z"
            };

            if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
                return Ok(dt.with_timezone(&Utc));
            }
        }

        // Couldn't parse as rfc3339 - fall back to NaiveDateTime.
        NaiveDateTime::try_from(value).map(|dt| Utc.from_utc_datetime(&dt))
    }
}

impl TryFrom<ValueOwned> for DateTime<Local> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Local>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let utc_dt = DateTime::<Utc>::try_from(value)?;
        Ok(utc_dt.with_timezone(&Local))
    }
}

impl TryFrom<ValueOwned> for DateTime<FixedOffset> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Local>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        Self::parse_from_rfc3339(s)
            .or_else(|_| Self::parse_from_str(s, "%F %T%.f%:z"))
            .map_err(|err| Error::Sqlite(err.to_string().into()))
    }
}

impl TryFrom<ValueOwned> for serde_json::Value {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let slf = match value {
            ValueOwned::Null => serde_json::Value::Null,
            ValueOwned::Integer(i) => serde_json::Value::from(i),
            ValueOwned::Real(r) => serde_json::Value::from(r),
            ValueOwned::Text(s) => serde_json::from_str(s.as_str())
                .map_err(|err| Error::Sqlite(err.to_string().into()))?,
            ValueOwned::Blob(b) => {
                serde_json::from_slice(&b).map_err(|err| Error::Sqlite(err.to_string().into()))?
            }
        };
        Ok(slf)
    }
}

impl TryFrom<ValueOwned> for Option<url::Url> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => {
                if s.is_empty() {
                    return Ok(None);
                }
                match url::Url::parse(&s) {
                    Ok(url) => Ok(Some(url)),
                    Err(_) => Err(Error::Sqlite(
                        "Cannot parse URL from given input: {s}".into(),
                    )),
                }
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse URL from TEXT column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for url::Url {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => match url::Url::parse(&s) {
                Ok(url) => Ok(url),
                Err(_) => Err(Error::Sqlite(
                    "Cannot parse URL from given input: {s}".into(),
                )),
            },
            _ => Err(Error::Sqlite(
                "Cannot only parse URL from TEXT column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for Option<uuid::Uuid> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => {
                if b.is_empty() {
                    return Ok(None);
                }
                let bytes = <[u8; 16]>::try_from(b.as_slice()).map_err(|_| {
                    Error::Sqlite("Invalid data in BLOB to be able to convert to UUID".into())
                })?;
                Ok(Some(uuid::Uuid::from_u128(u128::from_be_bytes(bytes))))
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse UUID from BLOB column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for uuid::Uuid {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => {
                let bytes = <[u8; 16]>::try_from(b.as_slice()).map_err(|_| {
                    Error::Sqlite("Invalid data in BLOB to be able to convert to UUID".into())
                })?;
                Ok(uuid::Uuid::from_u128(u128::from_be_bytes(bytes)))
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse UUID from BLOB column".into(),
            )),
        }
    }
}

#[cfg(all(test, feature = "macros"))]
mod try_from_row_tests {
    use super::*;
    use crate::macros::TryFromRow;

    #[derive(Debug, PartialEq, TryFromRow)]
    struct Inner {
        a: String,
    }

    #[derive(Debug, PartialEq, TryFromRow)]
    struct Outer {
        name: String,
        #[column(from_i32)]
        count: i32,
        #[column(parse)]
        big: u64,
        #[column(skip)]
        skipped: i64,
        #[column(flatten)]
        inner: Inner,
    }

    fn make_row(cols: &[(&str, ValueOwned)]) -> Row<'static> {
        let names: Vec<String> = cols.iter().map(|(name, _)| (*name).to_string()).collect();
        let values: Vec<Option<ValueOwned>> =
            cols.iter().map(|(_, value)| Some(value.clone())).collect();
        let cols = Arc::new(ColIdx::from_names(&names));
        Row::Owned(RowOwned { values }, cols)
    }

    // Pin the target type so `TryFrom` resolves unambiguously (both `Inner` and `Outer`
    // implement it).
    fn try_outer(row: &mut Row<'_>) -> Result<Outer, Error> {
        std::convert::TryFrom::try_from(row)
    }

    #[test]
    fn try_from_row_maps_all_attribute_kinds() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(42)),
            ("big", ValueOwned::Text(u64::MAX.to_string())),
            ("skipped", ValueOwned::Integer(999)),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let outer = try_outer(&mut row).unwrap();
        assert_eq!(outer.name, "hi");
        assert_eq!(outer.count, 42);
        assert_eq!(outer.big, u64::MAX);
        // `skip` is never read from the row, so it must stay at its default.
        assert_eq!(outer.skipped, 0);
        assert_eq!(outer.inner.a, "inner");
    }

    #[test]
    fn try_from_row_i32_overflow_is_an_error_not_a_panic() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(i64::MAX)),
            ("big", ValueOwned::Text("1".into())),
            ("skipped", ValueOwned::Null),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let err = try_outer(&mut row).unwrap_err();
        assert!(
            matches!(err, Error::Sqlite(_)),
            "expected Sqlite error: {err:?}"
        );
    }

    #[test]
    fn try_from_row_parse_failure_is_an_error() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(1)),
            ("big", ValueOwned::Text("not-a-number".into())),
            ("skipped", ValueOwned::Null),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let err = try_outer(&mut row).unwrap_err();
        assert!(
            matches!(err, Error::Sqlite(_)),
            "expected Sqlite error: {err:?}"
        );
    }
}

#[cfg(test)]
mod owned_row_tests {
    use super::*;

    fn owned(values: Vec<Option<ValueOwned>>, names: &[&str]) -> (Row<'static>, Arc<ColIdx>) {
        let cols = Arc::new(ColIdx::from_names(
            &names
                .iter()
                .map(|n| (*n).to_string())
                .collect::<Vec<String>>(),
        ));
        (Row::Owned(RowOwned { values }, cols.clone()), cols)
    }

    #[test]
    fn a_column_can_only_be_read_once() {
        let (mut row, _) = owned(
            vec![
                Some(ValueOwned::Integer(1)),
                Some(ValueOwned::Integer(2)),
                Some(ValueOwned::Integer(3)),
            ],
            &["a", "b", "c"],
        );
        assert_eq!(row.try_get::<i64>("a").unwrap(), 1);
        assert!(
            matches!(row.try_get::<i64>("a"), Err(Error::QueryParams(_))),
            "a second read of the same column must be an error"
        );
        // a taken slot never touches the other columns of the row
        assert_eq!(row.try_get::<i64>("c").unwrap(), 3);
        assert_eq!(row.try_get::<i64>("b").unwrap(), 2);
    }

    #[test]
    #[should_panic(expected = "not found")]
    fn get_after_a_taken_slot_panics() {
        let (mut row, _) = owned(vec![Some(ValueOwned::Integer(1))], &["a"]);
        assert_eq!(row.get::<i64>("a"), 1);
        let _ = row.get::<i64>("a");
    }

    #[test]
    fn a_name_that_is_not_on_the_row_is_an_error() {
        let (mut row, _) = owned(vec![Some(ValueOwned::Integer(1))], &["a"]);
        assert!(
            matches!(row.try_get::<i64>("missing"), Err(Error::QueryParams(_))),
            "an unknown column must be an error"
        );
    }

    #[test]
    fn duplicate_names_map_to_the_first_column() {
        // rusqlite's own scan returns the first match, the index has to do the same.
        let cols = ColIdx::from_names(&["id".to_string(), "ts".to_string(), "id".to_string()]);
        assert_eq!(cols.index("id"), Some(0));
        assert_eq!(cols.index("ts"), Some(1));
        assert_eq!(cols.index("other"), None);
    }
}
