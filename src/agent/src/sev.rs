// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;

pub fn get_snp_host_data() -> Result<Vec<u8>> {
    match sev::firmware::guest::Firmware::open() {
        Ok(mut firmware) => {
            let report_data: [u8; 64] = [0; 64];
            match firmware.get_report(None, Some(report_data), Some(0)) {
                Ok(report) => Ok(report.host_data.to_vec()),
                Err(e) => Err(e.into()),
            }
        }
        Err(e) => Err(e.into()),
    }
}
