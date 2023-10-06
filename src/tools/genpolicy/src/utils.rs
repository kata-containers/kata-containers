// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use log::debug;

#[derive(Clone, Debug)]
pub struct Config {
    pub use_cache: bool,

    pub yaml_file: Option<String>,
    pub rules_file: String,
    pub infra_data_file: String,
    pub config_map_files: Option<Vec<String>>,

    pub silent_unsupported_fields: bool,
    pub raw_out: bool,
    pub base64_out: bool,
}

impl Config {
    pub fn new(
        use_cache: bool,
        yaml_file: Option<String>,
        input_files_path: Option<String>,
        config_map_files: &Vec<String>,
        silent_unsupported_fields: bool,
        raw_out: bool,
        base64_out: bool,
    ) -> Self {
        let mut input_path = ".".to_string();
        if let Some(path) = input_files_path {
            input_path = path.clone();
        }
        let rules_file = input_path.to_owned() + "/rules.rego";
        debug!("Rules file: {:?}", &rules_file);

        let infra_data_file = input_path.to_owned() + "/genpolicy-settings.json";
        debug!("Infra data file: {:?}", &infra_data_file);

        let cm_files = if !config_map_files.is_empty() {
            Some(config_map_files.clone())
        } else {
            None
        };

        Self {
            use_cache,
            yaml_file,
            rules_file,
            infra_data_file,
            config_map_files: cm_files,
            silent_unsupported_fields,
            raw_out,
            base64_out,
        }
    }
}
