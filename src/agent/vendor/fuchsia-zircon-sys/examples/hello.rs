// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate fuchsia_zircon_sys as zircon_sys;

pub fn main() {
    let time = unsafe { zircon_sys::zx_time_get(zircon_sys::ZX_CLOCK_MONOTONIC) };
    println!("before sleep, time = {}", time);
    unsafe { zircon_sys::zx_nanosleep(zircon_sys::zx_deadline_after(1000_000_000)); }
    let time = unsafe { zircon_sys::zx_time_get(zircon_sys::ZX_CLOCK_MONOTONIC) };
    println!("after sleep, time = {}", time);
}

