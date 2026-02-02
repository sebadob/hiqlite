use hiqlite_derive::CacheVariants;

#[derive(Debug, CacheVariants)]
pub enum Cache {
    Intern,
    Extern,
}
