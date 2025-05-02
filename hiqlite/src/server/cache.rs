use crate::cache_idx::CacheIndex;

#[derive(Debug, strum::EnumIter)]
pub enum Cache {
    Intern,
    Extern,
}

impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}
