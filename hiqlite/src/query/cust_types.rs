use crate::query::rows::ValueOwned;
use crate::{Error, Param};
use rusqlite::types::{FromSqlResult, ValueRef};
use std::fmt::{Debug, Display, Write};
use tracing::error;

#[derive(Debug, Clone, PartialEq)]
pub struct VecText<const S: char>(String);

impl<const S: char> From<VecText<S>> for Param {
    fn from(value: VecText<S>) -> Self {
        Self::Text(value.0)
    }
}

impl<const S: char> From<&VecText<S>> for Param {
    fn from(value: &VecText<S>) -> Self {
        Self::Text(value.0.clone())
    }
}

impl<const S: char> rusqlite::types::FromSql for VecText<S> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let slf = match value {
            ValueRef::Null => Self(String::default()),
            ValueRef::Text(v) => Self(String::from_utf8_lossy(v).to_string()),
            _ => {
                error!("Can only parse VecLf from a TEXT column");
                Self(String::default())
            }
        };
        Ok(slf)
    }
}

impl<const S: char> TryFrom<ValueOwned> for VecText<S> {
    type Error = Error;

    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let slf = match value {
            ValueOwned::Null => Self(String::default()),
            ValueOwned::Text(v) => Self(v),
            _ => {
                error!("Can only parse VecLf from a TEXT column");
                Self(String::default())
            }
        };
        Ok(slf)
    }
}

impl<const S: char, T> TryFrom<VecText<S>> for Vec<T>
where
    T: Debug + Display + for<'a> TryFrom<&'a str>,
{
    type Error = Error;

    fn try_from(value: VecText<S>) -> Result<Self, Self::Error> {
        value.into_vec()
    }
}

impl<const S: char, T> TryFrom<VecText<S>> for Option<Vec<T>>
where
    T: Debug + Display + for<'a> TryFrom<&'a str>,
{
    type Error = Error;

    fn try_from(value: VecText<S>) -> Result<Self, Self::Error> {
        value.into_vec_opt()
    }
}

impl<const S: char> VecText<S> {
    pub fn new<T>(value: &[T]) -> Result<VecText<S>, Error>
    where
        T: Debug + Display,
    {
        // make a guess for preallocation
        let mut s = String::with_capacity(value.len() * 2);
        for item in value {
            write!(s, "{item}{S}")?;
        }
        Ok(VecText(s))
    }

    #[inline]
    pub fn into_vec<T>(self) -> Result<Vec<T>, Error>
    where
        T: for<'a> TryFrom<&'a str>,
    {
        let mut res: Vec<T> = Vec::new();
        for line in self.0.split(S) {
            if line.is_empty() {
                continue;
            }
            let t = T::try_from(line)
                .map_err(|_| Error::Error("Cannot convert VecLf into Vec<_>".into()))?;
            res.push(t);
        }
        Ok(res)
    }

    #[inline]
    pub fn into_vec_opt<T>(self) -> Result<Option<Vec<T>>, Error>
    where
        T: for<'a> TryFrom<&'a str>,
    {
        if self.0.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.into_vec()?))
        }
    }

    #[inline]
    pub fn parse<T>(self) -> Result<Vec<T>, Error>
    where
        T: ::std::str::FromStr,
    {
        let mut res: Vec<T> = Vec::new();
        for line in self.0.split(S) {
            if line.is_empty() {
                continue;
            }
            let t: T = line
                .parse()
                .map_err(|_| Error::Error("Cannot convert VecLf into Vec<_>".into()))?;
            res.push(t);
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_text() {
        // make sure conversions work as expected with the generic args

        let v: VecText<'\n'> = VecText::new(&["Entry 1", "Entry 2", "And another one"]).unwrap();
        // parse() will work here as well. The different impls just provide more flexibility.
        let r = v.into_vec::<String>().unwrap();
        assert_eq!(
            vec![
                "Entry 1".to_string(),
                "Entry 2".to_string(),
                "And another one".to_string()
            ],
            r
        );

        let v: VecText<','> = VecText::new(&[1, 2, -3]).unwrap();
        let r = v.parse::<i32>().unwrap();
        assert_eq!(&[1, 2, -3], r.as_slice());

        let v: VecText<';'> = VecText::new(&[1, 2, 13]).unwrap();
        let r = v.parse::<u8>().unwrap();
        assert_eq!(&[1, 2, 13], r.as_slice());
    }
}
