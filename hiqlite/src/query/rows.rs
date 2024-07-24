use crate::Error;
use serde::{Deserialize, Serialize};

struct TestEntity {
    pub id: i64,
    pub name: String,
}

impl<'r> From<Row<'r>> for TestEntity {
    fn from(mut row: Row<'r>) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
        }
    }
}

#[derive(Debug)]
pub enum Row<'a> {
    Borrowed(&'a rusqlite::Row<'a>),
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnOwned {
    // TODO find a way to include all the column names only once at the very top level and
    // somehow get a reference of them into a `From<_>` impl, probably with a new Trait.
    pub(crate) name: String,
    pub(crate) value: ValueOwned,
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
            v => i64::try_from(v).map(|r| Some(r)),
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
            v => f64::try_from(v).map(|r| Some(r)),
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
            v => String::try_from(v).map(|r| Some(r)),
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
            v => Vec::try_from(v).map(|r| Some(r)),
        }
    }
}
