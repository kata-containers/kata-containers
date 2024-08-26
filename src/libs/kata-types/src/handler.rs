// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use anyhow::{Result, anyhow};
use std::collections::hash_map::Entry;

/// Generic manager to manage registered handlers.
pub struct HandlerManager<H> {
    handlers: HashMap<String, H>,
}

impl<H> Default for HandlerManager<H> 
where
    H: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<H> HandlerManager<H> 
where
    H: Clone,
{
    /// Create a new instance of `HandlerManager`.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler.
    pub fn add_handler(&mut self, ids: &[&str], handler: H) -> Result<()> {
        for &id in ids {
            match self.handlers.entry(id.to_string()) {
                Entry::Occupied(_) => {
                    return Err(anyhow!("handler for {} already exists", id));
                }
                Entry::Vacant(entry) => {
                    entry.insert(handler.clone());
                }
            }
        }
        Ok(())
    }

    /// Get handler with specified `id`.
    pub fn handler(&self, id: &str) -> Option<&H> {
        self.handlers.get(id)
    }

    /// Get names of registered handlers.
    pub fn get_handlers(&self) -> Vec<String> {
        self.handlers.keys().map(|v| v.to_string()).collect()
    }
}
