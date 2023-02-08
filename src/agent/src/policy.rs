// Copyright (c) 2022 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Result};
use tracing::instrument;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[instrument]
pub fn allowed(ep: &str) -> Result<()> {
    info!(sl!(), "allowed({}) starting", ep);

    info!(sl!(), "allowed({}) returning success", ep);
    Ok(())
}
