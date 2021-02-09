// Copyright © 2016, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
use super::*;
STRUCT!{struct PROPVARIANT {
    vt: VARTYPE,
    wReserved1: WORD,
    wReserved2: WORD,
    wReserved3: WORD,
    data: [u8; 16],
}}
