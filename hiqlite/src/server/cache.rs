use num_derive::ToPrimitive;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

#[derive(Debug, Serialize, Deserialize, EnumIter, ToPrimitive)]
pub enum Cache {
    Intern,
    Extern,
}
