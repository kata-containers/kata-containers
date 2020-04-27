// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(bare_trait_objects)]

pub mod agent;
pub mod agent_ttrpc;
pub mod health;
pub mod health_ttrpc;
pub mod oci;
pub mod types;
pub mod empty;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
