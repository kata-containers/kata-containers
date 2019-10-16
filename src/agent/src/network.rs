// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use protocols::types::{Interface, Route};
use std::collections::HashMap;

// Network fully describes a sandbox network with its interfaces, routes and dns
// related information.
#[derive(Debug, Default)]
pub struct Network {
    ifaces: HashMap<String, Interface>,
    routes: Vec<Route>,
    dns: Vec<String>,
}

impl Network {
    pub fn new() -> Network {
        Network {
            ifaces: HashMap::new(),
            routes: Vec::new(),
            dns: Vec::new(),
        }
    }

    pub fn set_dns(&mut self, dns: String) {
        self.dns.push(dns);
    }
}
