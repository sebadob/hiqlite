use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use strum::IntoEnumIterator;

#[derive(Debug, Serialize, Deserialize, strum::EnumIter, num_derive::ToPrimitive)]
enum Idx {
    Users,
    Groups,
    Scopes,
}

fn cache_idx<T>()
where
    T: Debug + Serialize + for<'a> Deserialize<'a> + IntoEnumIterator + ToPrimitive,
{
    for item in T::iter() {
        println!("{:?} -> {}", item, item.to_usize().unwrap());
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::store::state_machine::memory::test::{cache_idx, Idx};
//     use strum::IntoEnumIterator;
//
//     #[test]
//     fn test_cache_idx_generic() {
//         cache_idx::<Idx>();
//
//         // let items =
//         // println!("items: {:?}", items);
//         // let idx = Idx::try_from(1).unwrap();
//         // println!("iems from: {:?}", idx);
//         // }
//
//         todo!()
//         // cache_idx::<Idx>();
//     }
// }
