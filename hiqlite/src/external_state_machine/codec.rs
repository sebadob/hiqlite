//! Dependency-free byte codec for durable external-state-machine values.
//!
//! The layout intentionally matches the legacy bincode configuration
//! (little-endian, fixed-width integers, `u64` length prefixes, `u8` option tags)
//! for every supported shape, so databases written by earlier releases decode
//! unchanged.

/// Stable byte encoding for values that the external state machine persists,
/// such as the caller's commit coordinate.
///
/// Implementations are provided for integers, `bool`, `()`, byte arrays,
/// `Vec<u8>`, `String`, `Option<T>`, and tuples of up to four codec values.
/// Structs whose fields all implement `ExternalCodec` can use
/// [`external_codec!`](crate::external_codec):
///
/// ```
/// #[derive(Debug, Clone, PartialEq, Eq)]
/// pub struct LogCoordinate {
///     pub term: u64,
///     pub node_id: u64,
///     pub index: u64,
/// }
///
/// hiqlite::external_codec!(LogCoordinate { term, node_id, index });
/// ```
///
/// The encoding must stay backward-decodable for every retained checkpoint,
/// receipt, and snapshot.
pub trait ExternalCodec: Sized {
    /// Appends the encoded value to `out`.
    fn encode_into(&self, out: &mut Vec<u8>);

    /// Decodes one value from the front of `input`, advancing it past the
    /// consumed bytes.
    fn decode_from(input: &mut &[u8]) -> Result<Self, String>;

    /// Encodes the value into a new buffer.
    fn to_external_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out);
        out
    }

    /// Decodes a value that must consume `bytes` exactly.
    fn from_external_bytes(mut bytes: &[u8]) -> Result<Self, String> {
        let value = Self::decode_from(&mut bytes)?;
        if !bytes.is_empty() {
            return Err(format!("{} trailing bytes", bytes.len()));
        }
        Ok(value)
    }
}

/// Splits `len` bytes off the front of `input`.
fn take<'a>(input: &mut &'a [u8], len: usize) -> Result<&'a [u8], String> {
    if input.len() < len {
        return Err(format!(
            "unexpected end of input: need {len} bytes, have {}",
            input.len()
        ));
    }
    let (head, tail) = input.split_at(len);
    *input = tail;
    Ok(head)
}

fn decode_len(input: &mut &[u8]) -> Result<usize, String> {
    let len = u64::decode_from(input)?;
    let len = usize::try_from(len).map_err(|_| format!("length {len} exceeds usize"))?;
    if len > input.len() {
        return Err(format!(
            "length prefix {len} exceeds remaining {} bytes",
            input.len()
        ));
    }
    Ok(len)
}

macro_rules! int_codec {
    ($($ty:ty),*) => {$(
        impl ExternalCodec for $ty {
            fn encode_into(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }

            fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
                let bytes = take(input, size_of::<$ty>())?;
                Ok(<$ty>::from_le_bytes(bytes.try_into().expect("exact length")))
            }
        }
    )*};
}

int_codec!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl ExternalCodec for () {
    fn encode_into(&self, _out: &mut Vec<u8>) {}

    fn decode_from(_input: &mut &[u8]) -> Result<Self, String> {
        Ok(())
    }
}

impl ExternalCodec for bool {
    fn encode_into(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }

    fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
        match u8::decode_from(input)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("invalid bool byte {other}")),
        }
    }
}

impl<const N: usize> ExternalCodec for [u8; N] {
    fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self);
    }

    fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
        Ok(take(input, N)?.try_into().expect("exact length"))
    }
}

impl ExternalCodec for Vec<u8> {
    fn encode_into(&self, out: &mut Vec<u8>) {
        (self.len() as u64).encode_into(out);
        out.extend_from_slice(self);
    }

    fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
        let len = decode_len(input)?;
        Ok(take(input, len)?.to_vec())
    }
}

impl ExternalCodec for String {
    fn encode_into(&self, out: &mut Vec<u8>) {
        (self.len() as u64).encode_into(out);
        out.extend_from_slice(self.as_bytes());
    }

    fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
        let len = decode_len(input)?;
        String::from_utf8(take(input, len)?.to_vec()).map_err(|err| err.to_string())
    }
}

impl<T: ExternalCodec> ExternalCodec for Option<T> {
    fn encode_into(&self, out: &mut Vec<u8>) {
        match self {
            None => out.push(0),
            Some(value) => {
                out.push(1);
                value.encode_into(out);
            }
        }
    }

    fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
        match u8::decode_from(input)? {
            0 => Ok(None),
            1 => Ok(Some(T::decode_from(input)?)),
            other => Err(format!("invalid option tag {other}")),
        }
    }
}

macro_rules! tuple_codec {
    ($($name:ident),+) => {
        impl<$($name: ExternalCodec),+> ExternalCodec for ($($name,)+) {
            #[allow(non_snake_case)]
            fn encode_into(&self, out: &mut Vec<u8>) {
                let ($($name,)+) = self;
                $($name.encode_into(out);)+
            }

            fn decode_from(input: &mut &[u8]) -> Result<Self, String> {
                Ok(($($name::decode_from(input)?,)+))
            }
        }
    };
}

tuple_codec!(A);
tuple_codec!(A, B);
tuple_codec!(A, B, C);
tuple_codec!(A, B, C, D);

/// Implements [`ExternalCodec`](crate::external_state_machine::ExternalCodec)
/// for a struct by encoding the listed fields in order.
///
/// List every field exactly once, in declaration order, to stay byte-compatible
/// with the legacy bincode encoding of the same struct.
///
/// ```
/// #[derive(Debug, Clone, PartialEq, Eq)]
/// pub struct LogCoordinate {
///     pub term: u64,
///     pub node_id: u64,
///     pub index: u64,
/// }
///
/// hiqlite::external_codec!(LogCoordinate { term, node_id, index });
/// ```
#[macro_export]
macro_rules! external_codec {
    ($ty:ty { $($field:ident),+ $(,)? }) => {
        impl $crate::external_state_machine::ExternalCodec for $ty {
            fn encode_into(&self, out: &mut ::std::vec::Vec<u8>) {
                $($crate::external_state_machine::ExternalCodec::encode_into(&self.$field, out);)+
            }

            fn decode_from(
                input: &mut &[u8],
            ) -> ::std::result::Result<Self, ::std::string::String> {
                ::std::result::Result::Ok(Self {
                    $($field: $crate::external_state_machine::ExternalCodec::decode_from(input)?,)+
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    fn legacy<T: Serialize>(value: &T) -> Vec<u8> {
        bincode::serde::encode_to_vec(value, bincode::config::legacy()).unwrap()
    }

    fn assert_compat<T>(value: T)
    where
        T: ExternalCodec + Serialize + PartialEq + std::fmt::Debug,
    {
        let bytes = value.to_external_bytes();
        assert_eq!(bytes, legacy(&value), "legacy bincode bytes differ");
        assert_eq!(T::from_external_bytes(&bytes).unwrap(), value);
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Coordinate {
        term: u64,
        node_id: u64,
        index: u64,
    }

    crate::external_codec!(Coordinate {
        term,
        node_id,
        index
    });

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Mixed {
        epoch: u32,
        leader: Option<u64>,
        tag: String,
        blob: Vec<u8>,
        digest: [u8; 4],
        flag: bool,
        signed: i64,
    }

    crate::external_codec!(Mixed {
        epoch,
        leader,
        tag,
        blob,
        digest,
        flag,
        signed,
    });

    #[test]
    fn matches_legacy_bincode() {
        assert_compat(());
        assert_compat(u64::MAX);
        assert_compat(-7i32);
        assert_compat(true);
        assert_compat((3u64, 9u64, 27u64));
        assert_compat(Some(5u16));
        assert_compat(None::<u16>);
        assert_compat("node-a".to_string());
        assert_compat(vec![1u8, 2, 3]);
        assert_compat(Coordinate {
            term: 7,
            node_id: 3,
            index: 123_456,
        });
        assert_compat(Mixed {
            epoch: 2,
            leader: Some(11),
            tag: "workspace".into(),
            blob: vec![0, 255, 7],
            digest: [9, 8, 7, 6],
            flag: true,
            signed: -42,
        });
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(u64::from_external_bytes(&[1, 2, 3]).is_err());
        assert!(u8::from_external_bytes(&[1, 2]).is_err());
        assert!(bool::from_external_bytes(&[2]).is_err());
        assert!(Option::<u8>::from_external_bytes(&[2, 0]).is_err());
        let mut huge = u64::MAX.to_le_bytes().to_vec();
        huge.push(0);
        assert!(Vec::<u8>::from_external_bytes(&huge).is_err());
        assert!(String::from_external_bytes(&[1, 0, 0, 0, 0, 0, 0, 0, 0xff]).is_err());
    }
}
