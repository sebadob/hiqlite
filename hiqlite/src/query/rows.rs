use crate::Error;
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
                // we always map expression results to strings
                ColumnType::Expr => {
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
    fn from_decl_type(typ: Option<&str>) -> Self {
        if let Some(t) = typ {
            match t {
                "INTEGER" => Self::Integer,
                "REAL" => Self::Real,
                "TEXT" => Self::Text,
                "BLOB" => Self::Blob,
                ct => unreachable!("unreachable column type: {}", ct),
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

impl TryFrom<ValueOwned> for i64 {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i),
            _ => Err(Error::Sqlite("Cannot convert into i64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for std::option::Option<i64> {
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

impl TryFrom<ValueOwned> for std::option::Option<f64> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => f64::try_from(v).map(Some),
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

impl TryFrom<ValueOwned> for std::option::Option<String> {
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

impl TryFrom<ValueOwned> for std::option::Option<Vec<u8>> {
    type Error = crate::Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => Vec::try_from(v).map(Some),
        }
    }
}
