// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::types::{Config, Options};
use anyhow::{anyhow, Result};
use oci::{Process as ociProcess, Root as ociRoot, Spec as ociSpec};
use protocols::oci::{
    Box as grpcBox, Linux as grpcLinux, LinuxCapabilities as grpcLinuxCapabilities,
    Process as grpcProcess, Root as grpcRoot, Spec as grpcSpec, User as grpcUser,
};
use rand::Rng;
use slog::{debug, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Length of a sandbox identifier
const SANDBOX_ID_LEN: u8 = 64;

const FILE_URI: &str = "file://";

// Length of the guests hostname
const MIN_HOSTNAME_LEN: u8 = 8;

// Name of the OCI configuration file found at the root of an OCI bundle.
const CONFIG_FILE: &str = "config.json";

lazy_static! {
    // Create a mutable hash map statically
    static ref SIGNALS: Arc<Mutex<HashMap<&'static str, u8>>> = {

        let mut m: HashMap<&'static str, u8> = HashMap::new();

        m.insert("SIGHUP", 1);
        m.insert("SIGINT", 2);
        m.insert("SIGQUIT", 3);
        m.insert("SIGILL", 4);
        m.insert("SIGTRAP", 5);
        m.insert("SIGABRT", 6);
        m.insert("SIGBUS", 7);
        m.insert("SIGFPE", 8);
        m.insert("SIGKILL", 9);
        m.insert("SIGUSR1", 10);
        m.insert("SIGSEGV", 11);
        m.insert("SIGUSR2", 12);
        m.insert("SIGPIPE", 13);
        m.insert("SIGALRM", 14);
        m.insert("SIGTERM", 15);
        m.insert("SIGSTKFLT", 16);

        // XXX:
        m.insert("SIGCHLD", 17);
        m.insert("SIGCLD", 17);

        m.insert("SIGCONT", 18);
        m.insert("SIGSTOP", 19);
        m.insert("SIGTSTP", 20);
        m.insert("SIGTTIN", 21);
        m.insert("SIGTTOU", 22);
        m.insert("SIGURG", 23);
        m.insert("SIGXCPU", 24);
        m.insert("SIGXFSZ", 25);
        m.insert("SIGVTALRM", 26);
        m.insert("SIGPROF", 27);
        m.insert("SIGWINCH", 28);
        m.insert("SIGIO", 29);
        m.insert("SIGPWR", 30);
        m.insert("SIGSYS", 31);

        Arc::new(Mutex::new(m))
    };
}

pub fn signame_to_signum(name: &str) -> Result<u8> {
    if name == "" {
        return Err(anyhow!("invalid signal"));
    }

    match name.parse::<u8>() {
        Ok(n) => return Ok(n),

        // "fall through" on error as we assume the name is not a number, but
        // a signal name.
        Err(_) => (),
    }

    let mut search_term: String;

    if name.starts_with("SIG") {
        search_term = name.to_string();
    } else {
        search_term = format!("SIG{}", name);
    }

    search_term = search_term.to_uppercase();

    // Access the hashmap
    let signals_ref = SIGNALS.clone();
    let m = signals_ref.lock().unwrap();

    match m.get(&*search_term) {
        Some(value) => Ok(*value),
        None => Err(anyhow!(format!("invalid signal name: {:?}", name))),
    }
}

// Convert a human time fornat (like "2s") into the equivalent number
// of nano seconds.
pub fn human_time_to_ns(human_time: &str) -> Result<i64> {
    if human_time == "" || human_time == "0" {
        return Ok(0);
    }

    let d: humantime::Duration = human_time
        .parse::<humantime::Duration>()
        .map_err(|e| anyhow!(e))?
        .into();

    Ok(d.as_nanos() as i64)
}

// Look up the specified option name and return its value.
//
// - The function looks for the appropriate option value in the specified
//   'args' first.
// - 'args' is assumed to be a space-separated set of "name=value" pairs).
// - If not found in the args, the function looks in the global options hash.
// - If found in neither location, certain well-known options are auto-generated.
// - All other options values default to an empty string.
// - All options are saved in the global hash before being returned for future
//   use.
pub fn get_option(name: &str, options: &mut Options, args: &str) -> String {
    let words: Vec<&str> = args.split_whitespace().collect();

    for word in words {
        let fields: Vec<String> = word.split("=").map(|s| s.to_string()).collect();

        if fields.len() < 2 {
            continue;
        }

        if fields[0] == "" {
            continue;
        }

        let key = fields[0].clone();

        let mut value = fields[1..].join("=");

        // Expand "spec=file:///some/where/config.json"
        if key == "spec" && value.starts_with(FILE_URI) {
            let spec_file = match uri_to_filename(&value) {
                Ok(file) => file,
                Err(e) => {
                    warn!(sl!(), "failed to handle spec file URI: {:}", e);

                    "".to_string()
                }
            };

            if spec_file != "" {
                value = match spec_file_to_string(spec_file) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(sl!(), "failed to load spec file: {:}", e);

                        "".to_string()
                    }
                };
            }
        }

        // Command args take priority over any previous value,
        // so update the global set of options for this and all
        // subsequent commands.
        options.insert(key, value);
    }

    // Explains briefly how the option value was determined
    let mut msg = "cached";

    // If the option exists in the hash, return it
    if let Some(value) = options.get(name) {
        debug!(sl!(), "using option {:?}={:?} ({})", name, value, msg);

        return value.to_string();
    }

    msg = "generated";

    // Handle option values that can be auto-generated
    let value = match name {
        "cid" => random_container_id(),
        "sid" => random_sandbox_id(),

        // Default to CID
        "exec_id" => {
            msg = "derived";
            //derived = true;

            match options.get("cid") {
                Some(value) => value.to_string(),
                None => "".to_string(),
            }
        }
        _ => "".to_string(),
    };

    debug!(sl!(), "using option {:?}={:?} ({})", name, value, msg);

    // Store auto-generated value
    options.insert(name.to_string(), value.to_string());

    value
}

pub fn generate_random_hex_string(len: u32) -> String {
    const CHARSET: &[u8] = b"abcdef0123456789";
    let mut rng = rand::thread_rng();

    let str: String = (0..len)
        .map(|_| {
            let idx = rng.gen_range(0, CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    str
}

pub fn random_sandbox_id() -> String {
    generate_random_hex_string(SANDBOX_ID_LEN as u32)
}

pub fn random_container_id() -> String {
    // Containers and sandboxes have same ID types
    random_sandbox_id()
}

fn config_file_from_bundle_dir(bundle_dir: &str) -> Result<String> {
    if bundle_dir == "" {
        return Err(anyhow!("missing bundle directory"));
    }

    let config_path = PathBuf::from(&bundle_dir).join(CONFIG_FILE);

    config_path
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}", e).context("failed to construct config file path"))
}

fn root_oci_to_grpc(bundle_dir: &str, root: &ociRoot) -> Result<grpcRoot> {
    let root_dir = root.path.clone();

    let path = if root_dir.starts_with("/") {
        root_dir.clone()
    } else {
        // Expand the root directory into an absolute value
        let abs_root_dir = PathBuf::from(&bundle_dir).join(&root_dir);

        abs_root_dir
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("{:?}", e).context("failed to construct bundle path"))?
    };

    let grpc_root = grpcRoot {
        Path: path,
        Readonly: root.readonly,
        unknown_fields: protobuf::UnknownFields::new(),
        cached_size: protobuf::CachedSize::default(),
    };

    Ok(grpc_root)
}

fn process_oci_to_grpc(p: &ociProcess) -> grpcProcess {
    let console_size = match &p.console_size {
        Some(s) => {
            let mut b = grpcBox::new();

            b.set_Width(s.width);
            b.set_Height(s.height);

            protobuf::SingularPtrField::some(b)
        }
        None => protobuf::SingularPtrField::none(),
    };

    let oom_score_adj: i64 = match p.oom_score_adj {
        Some(s) => s.into(),
        None => 0,
    };

    let mut user = grpcUser::new();
    user.set_UID(p.user.uid);
    user.set_GID(p.user.gid);
    user.set_AdditionalGids(p.user.additional_gids.clone());

    // FIXME: Implement RLimits OCI spec handling (copy from p.rlimits)
    //let rlimits = vec![grpcPOSIXRlimit::new()];
    let rlimits = protobuf::RepeatedField::new();

    // FIXME: Implement Capabilities OCI spec handling (copy from p.capabilities)
    let capabilities = grpcLinuxCapabilities::new();

    // FIXME: Implement Env OCI spec handling (copy from p.env)
    let env = protobuf::RepeatedField::new();

    grpcProcess {
        Terminal: p.terminal,
        ConsoleSize: console_size,
        User: protobuf::SingularPtrField::some(user),
        Args: protobuf::RepeatedField::from_vec(p.args.clone()),
        Env: env,
        Cwd: p.cwd.clone(),
        Capabilities: protobuf::SingularPtrField::some(capabilities),
        Rlimits: rlimits,
        NoNewPrivileges: p.no_new_privileges,
        ApparmorProfile: p.apparmor_profile.clone(),
        OOMScoreAdj: oom_score_adj,
        SelinuxLabel: p.selinux_label.clone(),
        unknown_fields: protobuf::UnknownFields::new(),
        cached_size: protobuf::CachedSize::default(),
    }
}

fn oci_to_grpc(bundle_dir: &str, cid: &str, oci: &ociSpec) -> Result<grpcSpec> {
    let process = match &oci.process {
        Some(p) => protobuf::SingularPtrField::some(process_oci_to_grpc(&p)),
        None => protobuf::SingularPtrField::none(),
    };

    let root = match &oci.root {
        Some(r) => {
            let grpc_root = root_oci_to_grpc(bundle_dir, &r).map_err(|e| e)?;

            protobuf::SingularPtrField::some(grpc_root)
        }
        None => protobuf::SingularPtrField::none(),
    };

    // FIXME: Implement Linux OCI spec handling
    let linux = grpcLinux::new();

    if cid.len() < MIN_HOSTNAME_LEN as usize {
        return Err(anyhow!("container ID too short for hostname"));
    }

    // FIXME: Implement setting a custom (and unique!) hostname (requires uts ns setup)
    //let hostname = cid[0..MIN_HOSTNAME_LEN as usize].to_string();
    let hostname = "".to_string();

    let grpc_spec = grpcSpec {
        Version: oci.version.clone(),
        Process: process,
        Root: root,
        Hostname: hostname,
        Mounts: protobuf::RepeatedField::new(),
        Hooks: protobuf::SingularPtrField::none(),
        Annotations: HashMap::new(),
        Linux: protobuf::SingularPtrField::some(linux),
        Solaris: protobuf::SingularPtrField::none(),
        Windows: protobuf::SingularPtrField::none(),
        unknown_fields: protobuf::UnknownFields::new(),
        cached_size: protobuf::CachedSize::default(),
    };

    Ok(grpc_spec)
}

fn uri_to_filename(uri: &str) -> Result<String> {
    if !uri.starts_with(FILE_URI) {
        return Err(anyhow!(format!("invalid URI: {:?}", uri)));
    }

    let fields: Vec<&str> = uri.split(FILE_URI).collect();

    if fields.len() != 2 {
        return Err(anyhow!(format!("invalid URI: {:?}", uri)));
    }

    Ok(fields[1].to_string())
}

pub fn spec_file_to_string(spec_file: String) -> Result<String> {
    let oci_spec = ociSpec::load(&spec_file).map_err(|e| anyhow!(e))?;

    serde_json::to_string(&oci_spec).map_err(|e| anyhow!(e))
}

pub fn get_oci_spec_json(cfg: &Config) -> Result<String> {
    let spec_file = config_file_from_bundle_dir(&cfg.bundle_dir)?;

    spec_file_to_string(spec_file)
}

pub fn get_grpc_spec(options: &mut Options, cid: &str) -> Result<grpcSpec> {
    let bundle_dir = get_option("bundle-dir", options, "");

    let json_spec = get_option("spec", options, "");
    assert_ne!(json_spec, "");

    let oci_spec: ociSpec = serde_json::from_str(&json_spec).map_err(|e| anyhow!(e))?;

    Ok(oci_to_grpc(&bundle_dir, cid, &oci_spec)?)
}

pub fn str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let prefix = "hex:";

    if s.starts_with(prefix) {
        let hex_str = s.trim_start_matches(prefix);

        let decoded = hex::decode(hex_str).map_err(|e| anyhow!(e))?;

        Ok(decoded)
    } else {
        Ok(s.as_bytes().to_vec())
    }
}
