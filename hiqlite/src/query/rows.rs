use crate::Error;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Row<'a> {
    Borrowed(&'a rusqlite::Row<'a>),
    // TODO we could only include ValueOwned + ref to ColumnInfo -> smaller payloads
    Owned(RowOwned),
}

impl Row<'_> {
    pub fn get<T>(&mut self, idx: &str) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Row::Borrowed(b) => b.get_unwrap(idx),
            Row::Owned(o) => o.try_get(idx).unwrap(),
        }
    }

    pub fn try_get<T>(&mut self, idx: &str) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Self::Borrowed(r) => r.get(idx).map_err(Error::from),
            Self::Owned(o) => o.try_get(idx),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct RowOwned {
    pub(crate) columns: Vec<ColumnOwned>,
}

impl RowOwned {
    #[inline(always)]
    pub(crate) fn from_row_column(row: &rusqlite::Row, columns: &[ColumnInfo]) -> Self {
        let mut cols = Vec::with_capacity(columns.len());

        for (i, info) in columns.iter().enumerate() {
            let value = match info.typ {
                ColumnType::Expr => {
                    // returned expressions can be any type we don't know in advance
                    // TODO is there a nicer solution for this with the encapsulated type?
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
                ColumnType::Integer => row
                    .get(i)
                    .map(ValueOwned::Integer)
                    .unwrap_or(ValueOwned::Null),
                ColumnType::Real => row.get(i).map(ValueOwned::Real).unwrap_or(ValueOwned::Null),
                ColumnType::Text => row.get(i).map(ValueOwned::Text).unwrap_or(ValueOwned::Null),
                ColumnType::Blob => row.get(i).map(ValueOwned::Blob).unwrap_or(ValueOwned::Null),
            };

            cols.push(ColumnOwned {
                name: info.name.clone(),
                value,
            })
        }

        Self { columns: cols }
    }
}

impl RowOwned {
    /// # Panics
    /// If the type cannot be converted correctly
    pub fn get<T: TryFrom<ValueOwned, Error = crate::Error>>(&mut self, idx: &str) -> T {
        self.try_get(idx).unwrap()
    }

    /// TODO decide which version to use - this is with remove -> benchmark them!
    pub fn try_get<T: TryFrom<ValueOwned, Error = crate::Error>>(
        &mut self,
        idx: &str,
    ) -> Result<T, Error> {
        for i in 0..self.columns.len() {
            if self.columns[i].name == idx {
                // swap_remove is fine because we don't allow access by raw integer index
                return T::try_from(self.columns.swap_remove(i).value);
            }
        }

        Err(Error::QueryParams(
            format!("column '{}' not found", idx).into(),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub typ: ColumnType,
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
    #[inline(always)]
    fn from_decl_type(typ: Option<&str>) -> Self {
        // we will always match as few character as possible for improved speed
        if let Some(t) = typ {
            // We will check "correct" types first for better speed with correct definitions.
            // SQLITE will convert int, integer, text, blob, real to UPPERCASE on creation.
            if t.starts_with("INT") {
                return Self::Integer;
            }

            match t {
                "TEXT" => Self::Text,
                "BLOB" => Self::Blob,
                "REAL" => Self::Real,
                _ => {
                    // When the proper types don't exist, try check type-affinity.
                    // Type affinity matches will NOT be converted to uppercase automatically!

                    if t.is_empty() {
                        return Self::Blob;
                    }

                    // A new allocation is less expensive than checking each match twice.
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
                    // However, we should have caught anything here -> panic!() to catch user errors
                    // early is the better option to avoid hard to find bugs at runtime.
                    } else {
                        unreachable!("unreachable column type: {}", t)
                    }
                }
            }
        } else {
            Self::Expr
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnOwned {
    // TODO find a way to include all the column names only once at the very top level and
    // somehow get a reference of them into a `From<_>` impl, probably with a new Trait.
    pub(crate) name: String,
    pub(crate) value: ValueOwned,
}

impl ColumnOwned {
    #[inline(always)]
    pub(crate) fn mapping_cols_from_stmt(
        columns: Vec<rusqlite::Column>,
    ) -> Result<Vec<ColumnInfo>, Error> {
        let mut cols = Vec::with_capacity(columns.len());
        for col in columns {
            cols.push(ColumnInfo {
                name: col.name().to_string(),
                typ: ColumnType::from_decl_type(col.decl_type()),
            });
        }
        Ok(cols)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValueOwned {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl ValueOwned {
    fn try_as_str(&self) -> Result<&str, Error> {
        match self {
            ValueOwned::Text(s) => Ok(s.as_str()),
            _ => Err(Error::Sqlite("Cannot convert ValueOwned to &str".into())),
        }
    }
}

impl TryFrom<ValueOwned> for i64 {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i),
            _ => Err(Error::Sqlite("Cannot convert into i64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<i64> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => i64::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for f64 {
    type Error = Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Real(r) => Ok(r),
            _ => Err(Error::Sqlite("Cannot convert into f64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<f64> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => f64::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for bool {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i == 1),
            ValueOwned::Real(i) => Ok(i == 1.0),
            ValueOwned::Text(s) => Ok(s.as_str() == "true"),
            _ => Err(Error::Sqlite("Cannot convert into bool".into())),
        }
    }
}

impl TryFrom<ValueOwned> for String {
    type Error = Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => Ok(s),
            _ => Err(Error::Sqlite("Cannot convert into String".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<String> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => String::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for Vec<u8> {
    type Error = Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => Ok(b),
            _ => Err(Error::Sqlite("Cannot convert into Vec<u8>".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<Vec<u8>> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => Vec::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveDate {
    type Error = crate::Error;

    /// "YYYY-MM-DD" => ISO 8601 calendar date without timezone.
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
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let utc_dt = DateTime::<Utc>::try_from(value)?;
        Ok(utc_dt.with_timezone(&Local))
    }
}

impl TryFrom<ValueOwned> for DateTime<FixedOffset> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Local>`.
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        Self::parse_from_rfc3339(s)
            .or_else(|_| Self::parse_from_str(s, "%F %T%.f%:z"))
            .map_err(|err| Error::Sqlite(err.to_string().into()))
    }
}

impl TryFrom<ValueOwned> for serde_json::Value {
    type Error = crate::Error;

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
