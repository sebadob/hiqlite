use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rusqlite::types::{ToSqlOutput, Value};
use serde::{Deserialize, Serialize};

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
}

// impl ToSql for Param {
//     fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
//         todo!()
//     }
// }

impl Param {
    pub(crate) fn into_sql<'a>(self) -> ToSqlOutput<'a> {
        let value = match self {
            Param::Null => Value::Null,
            Param::Integer(i) => Value::Integer(i),
            Param::Real(r) => Value::Real(r),
            Param::Text(t) => Value::Text(t),
            Param::Blob(b) => Value::Blob(b),
        };
        ToSqlOutput::Owned(value)
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

// #[cfg(feature = "i128_blob")]
// #[cfg_attr(docsrs, doc(cfg(feature = "i128_blob")))]
// impl From<i128> for Param {
//     #[inline]
//     fn from(i: i128) -> Param {
//         // We store these biased (e.g. with the most significant bit flipped)
//         // so that comparisons with negative numbers work properly.
//         Param::Blob(i128::to_be_bytes(i ^ (1_i128 << 127)).to_vec())
//     }
// }

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
