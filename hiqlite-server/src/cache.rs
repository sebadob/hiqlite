use hiqlite::EnumIter;
use hiqlite::ToPrimitive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, EnumIter, ToPrimitive)]
pub enum Cache {
    Intern,
    Extern,
}
