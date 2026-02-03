// use crate::CacheVariants;
//
// #[derive(Debug)]
// #[allow(unused)]
// pub enum Cache {
//     Intern = 0,
//     Extern,
// }
//
// impl CacheVariants for Cache {
//     fn hiqlite_cache_index(&self) -> usize {
//         match self {
//             Self::Intern => 0,
//             Self::Extern => 1,
//         }
//     }
//
//     fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
//         &[(0, "Intern"), (1, "Extern")]
//     }
// }
