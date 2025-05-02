use crate::cache_idx::CacheIndex;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

#[derive(Debug, Serialize, Deserialize, EnumIter)]
pub enum Cache {
    Intern,
    Extern,
}

impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}
