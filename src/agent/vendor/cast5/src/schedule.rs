use crate::consts::{S5, S6, S7, S8};

macro_rules! get_i {
    ($x:expr, $i:expr) => {
        (($x[($i) / 4] >> (8 * (3 - (($i) % 4)))) & 0xff) as usize
    };
}

#[inline]
pub fn key_schedule(x: &mut [u32], z: &mut [u32], k: &mut [u32]) {
    z[0] = x[0]
        ^ S5[get_i!(x, 13)]
        ^ S6[get_i!(x, 15)]
        ^ S7[get_i!(x, 12)]
        ^ S8[get_i!(x, 14)]
        ^ S7[get_i!(x, 8)];
    z[1] = x[2]
        ^ S5[get_i!(z, 0)]
        ^ S6[get_i!(z, 2)]
        ^ S7[get_i!(z, 1)]
        ^ S8[get_i!(z, 3)]
        ^ S8[get_i!(x, 10)];
    z[2] = x[3]
        ^ S5[get_i!(z, 7)]
        ^ S6[get_i!(z, 6)]
        ^ S7[get_i!(z, 5)]
        ^ S8[get_i!(z, 4)]
        ^ S5[get_i!(x, 9)];
    z[3] = x[1]
        ^ S5[get_i!(z, 10)]
        ^ S6[get_i!(z, 9)]
        ^ S7[get_i!(z, 11)]
        ^ S8[get_i!(z, 8)]
        ^ S6[get_i!(x, 11)];
    k[0] = S5[get_i!(z, 8)]
        ^ S6[get_i!(z, 9)]
        ^ S7[get_i!(z, 7)]
        ^ S8[get_i!(z, 6)]
        ^ S5[get_i!(z, 2)];
    k[1] = S5[get_i!(z, 10)]
        ^ S6[get_i!(z, 11)]
        ^ S7[get_i!(z, 5)]
        ^ S8[get_i!(z, 4)]
        ^ S6[get_i!(z, 6)];
    k[2] = S5[get_i!(z, 12)]
        ^ S6[get_i!(z, 13)]
        ^ S7[get_i!(z, 3)]
        ^ S8[get_i!(z, 2)]
        ^ S7[get_i!(z, 9)];
    k[3] = S5[get_i!(z, 14)]
        ^ S6[get_i!(z, 15)]
        ^ S7[get_i!(z, 1)]
        ^ S8[get_i!(z, 0)]
        ^ S8[get_i!(z, 12)];

    x[0] = z[2]
        ^ S5[get_i!(z, 5)]
        ^ S6[get_i!(z, 7)]
        ^ S7[get_i!(z, 4)]
        ^ S8[get_i!(z, 6)]
        ^ S7[get_i!(z, 0)];
    x[1] = z[0]
        ^ S5[get_i!(x, 0)]
        ^ S6[get_i!(x, 2)]
        ^ S7[get_i!(x, 1)]
        ^ S8[get_i!(x, 3)]
        ^ S8[get_i!(z, 2)];
    x[2] = z[1]
        ^ S5[get_i!(x, 7)]
        ^ S6[get_i!(x, 6)]
        ^ S7[get_i!(x, 5)]
        ^ S8[get_i!(x, 4)]
        ^ S5[get_i!(z, 1)];
    x[3] = z[3]
        ^ S5[get_i!(x, 10)]
        ^ S6[get_i!(x, 9)]
        ^ S7[get_i!(x, 11)]
        ^ S8[get_i!(x, 8)]
        ^ S6[get_i!(z, 3)];
    k[4] = S5[get_i!(x, 3)]
        ^ S6[get_i!(x, 2)]
        ^ S7[get_i!(x, 12)]
        ^ S8[get_i!(x, 13)]
        ^ S5[get_i!(x, 8)];
    k[5] = S5[get_i!(x, 1)]
        ^ S6[get_i!(x, 0)]
        ^ S7[get_i!(x, 14)]
        ^ S8[get_i!(x, 15)]
        ^ S6[get_i!(x, 13)];
    k[6] = S5[get_i!(x, 7)]
        ^ S6[get_i!(x, 6)]
        ^ S7[get_i!(x, 8)]
        ^ S8[get_i!(x, 9)]
        ^ S7[get_i!(x, 3)];
    k[7] = S5[get_i!(x, 5)]
        ^ S6[get_i!(x, 4)]
        ^ S7[get_i!(x, 10)]
        ^ S8[get_i!(x, 11)]
        ^ S8[get_i!(x, 7)];

    z[0] = x[0]
        ^ S5[get_i!(x, 13)]
        ^ S6[get_i!(x, 15)]
        ^ S7[get_i!(x, 12)]
        ^ S8[get_i!(x, 14)]
        ^ S7[get_i!(x, 8)];
    z[1] = x[2]
        ^ S5[get_i!(z, 0)]
        ^ S6[get_i!(z, 2)]
        ^ S7[get_i!(z, 1)]
        ^ S8[get_i!(z, 3)]
        ^ S8[get_i!(x, 10)];
    z[2] = x[3]
        ^ S5[get_i!(z, 7)]
        ^ S6[get_i!(z, 6)]
        ^ S7[get_i!(z, 5)]
        ^ S8[get_i!(z, 4)]
        ^ S5[get_i!(x, 9)];
    z[3] = x[1]
        ^ S5[get_i!(z, 10)]
        ^ S6[get_i!(z, 9)]
        ^ S7[get_i!(z, 11)]
        ^ S8[get_i!(z, 8)]
        ^ S6[get_i!(x, 11)];
    k[8] = S5[get_i!(z, 3)]
        ^ S6[get_i!(z, 2)]
        ^ S7[get_i!(z, 12)]
        ^ S8[get_i!(z, 13)]
        ^ S5[get_i!(z, 9)];
    k[9] = S5[get_i!(z, 1)]
        ^ S6[get_i!(z, 0)]
        ^ S7[get_i!(z, 14)]
        ^ S8[get_i!(z, 15)]
        ^ S6[get_i!(z, 12)];
    k[10] = S5[get_i!(z, 7)]
        ^ S6[get_i!(z, 6)]
        ^ S7[get_i!(z, 8)]
        ^ S8[get_i!(z, 9)]
        ^ S7[get_i!(z, 2)];
    k[11] = S5[get_i!(z, 5)]
        ^ S6[get_i!(z, 4)]
        ^ S7[get_i!(z, 10)]
        ^ S8[get_i!(z, 11)]
        ^ S8[get_i!(z, 6)];

    x[0] = z[2]
        ^ S5[get_i!(z, 5)]
        ^ S6[get_i!(z, 7)]
        ^ S7[get_i!(z, 4)]
        ^ S8[get_i!(z, 6)]
        ^ S7[get_i!(z, 0)];
    x[1] = z[0]
        ^ S5[get_i!(x, 0)]
        ^ S6[get_i!(x, 2)]
        ^ S7[get_i!(x, 1)]
        ^ S8[get_i!(x, 3)]
        ^ S8[get_i!(z, 2)];
    x[2] = z[1]
        ^ S5[get_i!(x, 7)]
        ^ S6[get_i!(x, 6)]
        ^ S7[get_i!(x, 5)]
        ^ S8[get_i!(x, 4)]
        ^ S5[get_i!(z, 1)];
    x[3] = z[3]
        ^ S5[get_i!(x, 10)]
        ^ S6[get_i!(x, 9)]
        ^ S7[get_i!(x, 11)]
        ^ S8[get_i!(x, 8)]
        ^ S6[get_i!(z, 3)];
    k[12] = S5[get_i!(x, 8)]
        ^ S6[get_i!(x, 9)]
        ^ S7[get_i!(x, 7)]
        ^ S8[get_i!(x, 6)]
        ^ S5[get_i!(x, 3)];
    k[13] = S5[get_i!(x, 10)]
        ^ S6[get_i!(x, 11)]
        ^ S7[get_i!(x, 5)]
        ^ S8[get_i!(x, 4)]
        ^ S6[get_i!(x, 7)];
    k[14] = S5[get_i!(x, 12)]
        ^ S6[get_i!(x, 13)]
        ^ S7[get_i!(x, 3)]
        ^ S8[get_i!(x, 2)]
        ^ S7[get_i!(x, 8)];
    k[15] = S5[get_i!(x, 14)]
        ^ S6[get_i!(x, 15)]
        ^ S7[get_i!(x, 1)]
        ^ S8[get_i!(x, 0)]
        ^ S8[get_i!(x, 13)];
}
