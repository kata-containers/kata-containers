// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use log::info;

pub struct InOutFiles {
    pub yaml_file: Option<String>,
    pub rules_file: String,
    pub infra_data_file: String,
    pub output_policy_file: Option<String>,
    pub config_map_files: Option<Vec<String>>,
}

impl InOutFiles {
    pub fn new(
        yaml_file: Option<String>,
        input_files_path: Option<String>,
        output_policy_file: Option<String>,
        config_map_files: &Vec<String>,
    ) -> Self {
        let mut input_path = ".".to_string();
        if let Some(path) = input_files_path {
            input_path = path.clone();
        }
        let rules_file = input_path.to_owned() + "/rules.rego";
        info!("Rules file: {:?}", &rules_file);

        let infra_data_file = input_path.to_owned() + "/data.json";
        info!("Infra data file: {:?}", &infra_data_file);

        let cm_files = if !config_map_files.is_empty() {
            Some(config_map_files.clone())
        } else {
            None
        };

        Self {
            yaml_file,
            rules_file,
            infra_data_file,
            output_policy_file,
            config_map_files: cm_files,
        }
    }
}
