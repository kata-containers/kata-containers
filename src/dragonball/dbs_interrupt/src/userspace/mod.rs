// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod ioapic;
#[cfg(feature = "split-legacy-irq")]
pub mod legacy_irq;
pub mod manager;

use super::*;
