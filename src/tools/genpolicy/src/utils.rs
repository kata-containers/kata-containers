// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::settings;
use clap::Parser;

#[derive(Debug, Parser)]
struct CommandLineOptions {
    #[clap(
        short,
        long,
        help = "Kubernetes input/output YAML file path. stdin/stdout get used if this option is not specified."
    )]
    yaml_file: Option<String>,

    #[clap(
        short,
        long,
        help = "Optional Kubernetes config map YAML input file path"
    )]
    config_map_file: Option<String>,

    #[clap(
        short = 'p',
        long,
        default_value_t = String::from("rules.rego"),
        help = "Path to rego rules file"
    )]
    rego_rules_path: String,

    #[clap(
        short = 'j',
        long,
        default_value_t = String::from("genpolicy-settings.json"),
        help = "Path to genpolicy settings file"
    )]
    json_settings_path: String,

    #[clap(
        short,
        long,
        help = "Create and use a cache of container image layer contents and dm-verity information (in ./layers_cache/)"
    )]
    use_cached_files: bool,

    #[clap(
        short,
        long,
        help = "Print the output Rego policy text to standard output"
    )]
    raw_out: bool,

    #[clap(
        short,
        long,
        help = "Print the base64 encoded output Rego policy to standard output"
    )]
    base64_out: bool,

    #[clap(
        short,
        long,
        help = "Ignore unsupported input Kubernetes YAML fields. This is not recommeded unless you understand exactly how genpolicy works!"
    )]
    silent_unsupported_fields: bool,

    #[clap(
        short = 'd',
        long,
        help = "If specified, will use existing containerd service to pull container images. This option is only supported on Linux",
        // from https://docs.rs/clap/4.1.8/clap/struct.Arg.html#method.default_missing_value
        default_missing_value = "/var/run/containerd/containerd.sock", // used if flag is present but no value is given
        num_args = 0..=1,
        require_equals= true
    )]
    containerd_socket_path: Option<String>,

    #[clap(
        long,
        help = "Registry that uses plain HTTP. Can be passed more than once to configure multiple insecure registries."
    )]
    insecure_registry: Vec<String>,

    #[clap(
        long,
        help = "If specified, resources that have a runtimeClassName field defined will only receive a policy if the parameter is a prefix one of the given runtime class names."
    )]
    runtime_class_names: Vec<String>,

    #[clap(
        long,
        help = "Path to the layers cache file. This file is used to store the layers cache information. The default value is ./layers-cache.json.",
        default_missing_value = "./layers-cache.json",
        require_equals = true
    )]
    layers_cache_file_path: Option<String>,
    #[clap(short, long, help = "Print version information and exit")]
    version: bool,
}

/// Application configuration, derived from on command line parameters.
#[derive(Clone, Debug)]
pub struct Config {
    #[allow(dead_code)]
    pub use_cache: bool,
    pub insecure_registries: Vec<String>,
    pub runtime_class_names: Vec<String>,

    pub yaml_file: Option<String>,
    pub rego_rules_path: String,
    pub settings: settings::Settings,
    pub config_map_files: Option<Vec<String>>,

    pub silent_unsupported_fields: bool,
    pub raw_out: bool,
    pub base64_out: bool,
    pub containerd_socket_path: Option<String>,
    pub layers_cache_file_path: Option<String>,
    pub version: bool,
}

impl Config {
    pub fn new() -> Self {
        let args = CommandLineOptions::parse();

        let mut config_map_files = Vec::new();
        if let Some(config_map_file) = &args.config_map_file {
            config_map_files.push(config_map_file.clone());
        }

        let cm_files = if !config_map_files.is_empty() {
            Some(config_map_files.clone())
        } else {
            None
        };

        let mut layers_cache_file_path = args.layers_cache_file_path;
        // preserve backwards compatibility for only using the `use_cached_files` flag
        if args.use_cached_files && layers_cache_file_path.is_none() {
            layers_cache_file_path = Some(String::from("./layers-cache.json"));
        }

        let settings = settings::Settings::new(&args.json_settings_path);

        Self {
            use_cache: args.use_cached_files,
            insecure_registries: args.insecure_registry,
            runtime_class_names: args.runtime_class_names,
            yaml_file: args.yaml_file,
            rego_rules_path: args.rego_rules_path,
            settings,
            config_map_files: cm_files,
            silent_unsupported_fields: args.silent_unsupported_fields,
            raw_out: args.raw_out,
            base64_out: args.base64_out,
            containerd_socket_path: args.containerd_socket_path,
            layers_cache_file_path,
            version: args.version,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
