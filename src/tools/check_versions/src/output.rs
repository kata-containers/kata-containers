// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::OpenOptions;
use std::io::Write;
use anyhow::Result;

use crate::cli::Args;

pub fn write_output(content: String, args: &Args) -> Result<()> {
    match &args.outfile {
        Some(outfile) => {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(outfile.as_path())?;
            file.write_all(content.as_bytes())?;
        },
        None => ()
    };

    if !args.quiet {
        println!("{}", content);
    }

    Ok(())
}
