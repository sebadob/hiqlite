// Copyright 2025 Sebastian Dobe <sebastiandobe@mailbox.org>

pub mod embed {
    pub use rust_embed::{self, *};
}

/// Helper macro to created Owned Params which can be serialized and sent
/// over the Raft network between nodes.
#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            #[allow(unused_mut)]
            let mut params = Vec::with_capacity(2);
            $(
                params.push(hiqlite::Param::from($param));
            )*
            params
        }
    };
}
