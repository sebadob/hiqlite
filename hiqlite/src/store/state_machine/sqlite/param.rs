use std::borrow::Cow;

use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rusqlite::types::{ToSqlOutput, Value};
use serde::{Deserialize, Serialize};

use crate::Error;

use super::{
    transaction_env::{TransactionEnv, TransactionParamContext},
    transaction_variable::StmtColumn,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Param {
    /// The value is a `NULL` value.
    Null,
    /// The value is a signed integer.
    Integer(i64),
    /// The value is a floating point number.
    Real(f64),
    /// The value is a text string.
    Text(String),
    /// The value is a blob of data
    Blob(Vec<u8>),
    /// The value is a variable referencing the first row of a previous statement in a transaction.
    /// The key is the statement index and a column index.
    StmtOutputIndexed(usize, usize),
    /// The value is a variable referencing the first row of a previous statement in a transaction.
    /// The key is the statement index and a column name.
    StmtOutputNamed(usize, Cow<'static, str>),
}

impl Param {
    pub(crate) fn into_sql<'a>(self) -> ToSqlOutput<'a> {
        let value = match self {
            Param::Null => Value::Null,
            Param::Integer(i) => Value::Integer(i),
            Param::Real(r) => Value::Real(r),
            Param::Text(t) => Value::Text(t),
            Param::Blob(b) => Value::Blob(b),
            Param::StmtOutputNamed(..) | Param::StmtOutputIndexed(..) => {
                panic!("Param::StmtOutput is only valid inside transactions")
            }
        };
        ToSqlOutput::Owned(value)
    }

    pub(crate) fn into_sql_txn_ctx<'a>(
        self,
        mut ctx: TransactionParamContext,
    ) -> Result<ToSqlOutput<'a>, Cow<'static, str>> {
        let value = match self {
            Param::Null => Value::Null,
            Param::Integer(i) => Value::Integer(i),
            Param::Real(r) => Value::Real(r),
            Param::Text(t) => Value::Text(t),
            Param::Blob(b) => Value::Blob(b),
            Param::StmtOutputIndexed(stmt_index, column_index) => {
                ctx.lookup_statement_output_indexed(stmt_index, column_index)?
            }
            Param::StmtOutputNamed(stmt_index, column_name) => {
                ctx.lookup_statement_output_named(stmt_index, column_name)?
            }
        };
        Ok(ToSqlOutput::Owned(value))
    }
}

impl From<rusqlite::types::Null> for Param {
    #[inline]
    fn from(_: rusqlite::types::Null) -> Param {
        Param::Null
    }
}

impl From<bool> for Param {
    #[inline]
    fn from(i: bool) -> Param {
        Param::Integer(i as i64)
    }
}

impl From<isize> for Param {
    #[inline]
    fn from(i: isize) -> Param {
        Param::Integer(i as i64)
    }
}

impl From<uuid::Uuid> for Param {
    #[inline]
    fn from(id: uuid::Uuid) -> Param {
        // TODO need to be converted to BE bytes for correct ordering -> feature flag
        Param::Blob(id.as_bytes().to_vec())
    }
}

macro_rules! from_i64(
    ($t:ty) => (
        impl From<$t> for Param {
            #[inline]
            fn from(i: $t) -> Param {
                Param::Integer(i64::from(i))
            }
        }
    )
);

from_i64!(i8);
from_i64!(i16);
from_i64!(i32);
from_i64!(u8);
from_i64!(u16);
from_i64!(u32);

impl From<i64> for Param {
    #[inline]
    fn from(i: i64) -> Param {
        Param::Integer(i)
    }
}

impl From<f32> for Param {
    #[inline]
    fn from(f: f32) -> Param {
        Param::Real(f.into())
    }
}

impl From<f64> for Param {
    #[inline]
    fn from(f: f64) -> Param {
        Param::Real(f)
    }
}

impl From<&str> for Param {
    #[inline]
    fn from(s: &str) -> Param {
        Param::Text(s.to_string())
    }
}

impl From<String> for Param {
    #[inline]
    fn from(s: String) -> Param {
        Param::Text(s)
    }
}

impl From<&String> for Param {
    #[inline]
    fn from(s: &String) -> Param {
        Param::Text(s.clone())
    }
}

impl From<&[u8]> for Param {
    #[inline]
    fn from(v: &[u8]) -> Param {
        Param::Blob(v.to_vec())
    }
}

impl From<Vec<u8>> for Param {
    #[inline]
    fn from(v: Vec<u8>) -> Param {
        Param::Blob(v)
    }
}

impl From<DateTime<Utc>> for Param {
    /// UTC time => UTC RFC3339 timestamp
    /// ("YYYY-MM-DD HH:MM:SS.SSS+00:00")
    #[inline]
    fn from(value: DateTime<Utc>) -> Self {
        Param::Text(value.format("%F %T%.f%:z").to_string())
    }
}

impl From<DateTime<Local>> for Param {
    /// Local time => UTC RFC3339 timestamp
    /// ("YYYY-MM-DD HH:MM:SS.SSS+00:00")
    #[inline]
    fn from(value: DateTime<Local>) -> Self {
        Param::Text(value.with_timezone(&Utc).format("%F %T%.f%:z").to_string())
    }
}

impl From<DateTime<FixedOffset>> for Param {
    /// Date and time with time zone => RFC3339 timestamp
    /// ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM")
    #[inline]
    fn from(value: DateTime<FixedOffset>) -> Self {
        Param::Text(value.format("%F %T%.f%:z").to_string())
    }
}

impl From<NaiveDate> for Param {
    /// ISO 8601 calendar date without timezone => "YYYY-MM-DD"
    #[inline]
    fn from(value: NaiveDate) -> Self {
        Param::Text(value.format("%F").to_string())
    }
}

impl From<NaiveTime> for Param {
    /// ISO 8601 time without timezone => "HH:MM:SS.SSS"
    #[inline]
    fn from(value: NaiveTime) -> Self {
        Param::Text(value.format("%T%.f").to_string())
    }
}

impl From<NaiveDateTime> for Param {
    /// ISO 8601 combined date and time without timezone =>
    /// "YYYY-MM-DD HH:MM:SS.SSS"
    #[inline]
    fn from(value: NaiveDateTime) -> Self {
        Param::Text(value.format("%F %T%.f").to_string())
    }
}

impl From<serde_json::Value> for Param {
    #[inline]
    fn from(value: serde_json::Value) -> Self {
        Param::Text(value.to_string())
    }
}

impl<T> From<Option<T>> for Param
where
    T: Into<Param>,
{
    #[inline]
    fn from(v: Option<T>) -> Param {
        match v {
            Some(x) => x.into(),
            None => Param::Null,
        }
    }
}

impl<T> From<&Option<T>> for Param
where
    T: Clone + Into<Param>,
{
    #[inline]
    fn from(v: &Option<T>) -> Param {
        match v {
            Some(x) => x.clone().into(),
            None => Param::Null,
        }
    }
}

impl From<StmtColumn<usize>> for Param {
    fn from(value: StmtColumn<usize>) -> Self {
        Param::StmtOutputIndexed(value.stmt_index.0, value.column)
    }
}

impl From<StmtColumn<&'static str>> for Param {
    fn from(value: StmtColumn<&'static str>) -> Self {
        Param::StmtOutputNamed(value.stmt_index.0, value.column.into())
    }
}

impl From<StmtColumn<String>> for Param {
    fn from(value: StmtColumn<String>) -> Self {
        Param::StmtOutputNamed(value.stmt_index.0, value.column.into())
    }
}
