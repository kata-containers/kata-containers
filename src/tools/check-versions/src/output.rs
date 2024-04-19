// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;

use crate::cli::Args;
use crate::model::CheckResult;

pub fn output_results(results: &Vec<CheckResult>, args: &Args) -> Result<()> {
    for r in results {
        if r.success && r.up_to_date {
            if !args.suppress_uptodate {
                println!("[Up to Date] {}\n\tversion: {}", r.project_name, r.current_version)
            }

        }
        else if r.success && !r.up_to_date {
            if !args.suppress_outofdate {
                println!("[Out of Date] {}\n\tcurrent_version: {}\n\tlatest_version: {}",
                    r.project_name, r.current_version, r.latest_version);
            }
        } else {
            if !args.suppress_errors {
                match &r.message {
                    Some(msg) => println!("[Error] {}\n\tmessage: {}", r.project_name, msg),
                    None => ()
                }
            }
        }
    }

    Ok(())
}
