// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

pub use hiqlite_derive::FromRow;

pub mod embed {
    pub use rust_embed::{self, *};
}

/// Helper macro to created Owned Params which can be serialized and sent
/// over the Raft network between nodes.
#[macro_export]
macro_rules! params {
    ( $p1:expr ) => {
        {
            let mut params = Vec::with_capacity(1);
            params.push(hiqlite::Param::from($p1));
            params
        }
    };
    ( $p1:expr, $p2:expr ) => {
        {
            let mut params = Vec::with_capacity(2);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr ) => {
        {
            let mut params = Vec::with_capacity(3);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr, $p4:expr ) => {
        {
            let mut params = Vec::with_capacity(4);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr ) => {
        {
            let mut params = Vec::with_capacity(5);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr ) => {
        {
            let mut params = Vec::with_capacity(6);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr, $p7:expr ) => {
        {
            let mut params = Vec::with_capacity(7);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params
        }
    };
    ( $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr, $p7:expr, $p8:expr ) => {
        {
            let mut params = Vec::with_capacity(8);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(9);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(10);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(11);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(12);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(13);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(14);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(15);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(16);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(17);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(18);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(19);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
        $p20:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(20);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params.push(hiqlite::Param::from($p20));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
        $p20:expr,
        $p21:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(21);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params.push(hiqlite::Param::from($p20));
            params.push(hiqlite::Param::from($p21));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
        $p20:expr,
        $p21:expr,
        $p22:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(22);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params.push(hiqlite::Param::from($p20));
            params.push(hiqlite::Param::from($p21));
            params.push(hiqlite::Param::from($p22));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
        $p20:expr,
        $p21:expr,
        $p22:expr,
        $p23:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(23);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params.push(hiqlite::Param::from($p20));
            params.push(hiqlite::Param::from($p21));
            params.push(hiqlite::Param::from($p22));
            params.push(hiqlite::Param::from($p23));
            params
        }
    };
    (
        $p1:expr,
        $p2:expr,
        $p3:expr,
        $p4:expr,
        $p5:expr,
        $p6:expr,
        $p7:expr,
        $p8:expr,
        $p9:expr,
        $p10:expr,
        $p11:expr,
        $p12:expr,
        $p13:expr,
        $p14:expr,
        $p15:expr,
        $p16:expr,
        $p17:expr,
        $p18:expr,
        $p19:expr,
        $p20:expr,
        $p21:expr,
        $p22:expr,
        $p23:expr,
        $p24:expr,
    ) => {
        {
            let mut params = Vec::with_capacity(24);
            params.push(hiqlite::Param::from($p1));
            params.push(hiqlite::Param::from($p2));
            params.push(hiqlite::Param::from($p3));
            params.push(hiqlite::Param::from($p4));
            params.push(hiqlite::Param::from($p5));
            params.push(hiqlite::Param::from($p6));
            params.push(hiqlite::Param::from($p7));
            params.push(hiqlite::Param::from($p8));
            params.push(hiqlite::Param::from($p9));
            params.push(hiqlite::Param::from($p10));
            params.push(hiqlite::Param::from($p11));
            params.push(hiqlite::Param::from($p12));
            params.push(hiqlite::Param::from($p13));
            params.push(hiqlite::Param::from($p14));
            params.push(hiqlite::Param::from($p15));
            params.push(hiqlite::Param::from($p16));
            params.push(hiqlite::Param::from($p17));
            params.push(hiqlite::Param::from($p18));
            params.push(hiqlite::Param::from($p19));
            params.push(hiqlite::Param::from($p20));
            params.push(hiqlite::Param::from($p21));
            params.push(hiqlite::Param::from($p22));
            params.push(hiqlite::Param::from($p23));
            params.push(hiqlite::Param::from($p24));
            params
        }
    };
    ( $( $param:expr ),* ) => {
        {
            #[allow(unused_mut)]
            let mut params = Vec::with_capacity(25);
            $(
                params.push(hiqlite::Param::from($param));
            )*
            params
        }
    };
}

#[cfg(test)]
mod tests {
    use hiqlite_derive::FromRow;

    #[allow(dead_code)]
    #[derive(Debug, FromRow)]
    struct Test {
        pub id: i64,
        #[column(rename = "name_db")]
        name: String,
        desc: Option<String>,
        #[column(skip)]
        desc2: Option<String>,
        #[column(flatten)]
        sub: Sub,
    }

    #[allow(dead_code)]
    #[derive(Debug, FromRow)]
    struct Sub {
        id: i64,
        name: String,
    }

    #[test]
    fn from_row() {
        // Just make sure that the `From<&mut Row<'_>` impl compiles fine.
        #[allow(unused)]
        let t = Test {
            id: 13,
            name: "Name".to_string(),
            desc: Some("description".to_string()),
            desc2: None,
            sub: Sub {
                id: 27,
                name: "SubName".to_string(),
            },
        };
    }
}
