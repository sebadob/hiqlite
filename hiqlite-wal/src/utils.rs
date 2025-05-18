use crate::error::Error;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

pub const CHKSUM: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);

macro_rules! crc {
    ($input:expr) => {{
        crate::utils::CHKSUM.checksum($input).to_le_bytes()
    }};
}
pub(crate) use crc;

#[inline]
pub fn id_to_bin(id: u64, buf: &mut Vec<u8>) -> Result<(), Error> {
    buf.write_u64::<BigEndian>(id)?;
    Ok(())
}

#[inline]
pub fn bin_to_id(buf: &[u8]) -> Result<u64, Error> {
    Ok((&buf[0..8]).read_u64::<BigEndian>()?)
}

#[inline]
pub fn len_to_bin(id: u32, buf: &mut Vec<u8>) -> Result<(), Error> {
    buf.write_u32::<BigEndian>(id)?;
    Ok(())
}

#[inline]
pub fn bin_to_len(buf: &[u8]) -> Result<u32, Error> {
    Ok((&buf[0..4]).read_u32::<BigEndian>()?)
}

#[inline(always)]
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    Ok(bincode::serde::encode_to_vec(
        value,
        bincode::config::standard(),
    )?)
}

#[inline]
pub fn deserialize<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: for<'a> Deserialize<'a>,
{
    let (res, _) = bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::standard())?;
    Ok(res)
}
