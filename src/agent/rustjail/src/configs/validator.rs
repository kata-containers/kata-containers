// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::configs::Config;
use std::io::Result;

pub trait Validator {
    fn validate(&self, config: &Config) -> Result<()> {
        Ok(())
    }
}

pub struct ConfigValidator {}

impl Validator for ConfigValidator {}

impl ConfigValidator {
    fn new() -> Self {
        ConfigValidator {}
    }
}
