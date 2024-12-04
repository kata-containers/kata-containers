// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::image;
use crate::types::*;
use anyhow::{anyhow, Result};
use oci::{Root as ociRoot, Spec as ociSpec};
use oci_spec::runtime as oci;
use protocols::agent::{CopyFileRequest, CreateContainerRequest, SetPolicyRequest};
use protocols::oci::{
    Mount as ttrpcMount, Process as ttrpcProcess, Root as ttrpcRoot, Spec as ttrpcSpec,
};
use rand::Rng;
use safe_path::scoped_join;
use serde::de::DeserializeOwned;
use slog::{debug, warn};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// Length of a sandbox identifier
const SANDBOX_ID_LEN: u8 = 64;

const FILE_URI: &str = "file://";

// Length of the guests hostname
const MIN_HOSTNAME_LEN: u8 = 8;

// Name of the OCI configuration file found at the root of an OCI bundle.
const CONFIG_FILE: &str = "config.json";

// Path to OCI configuration template
const OCI_CONFIG_TEMPLATE: &str =
    "/opt/kata/share/defaults/kata-containers/agent-ctl/oci_config.json";

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
    if name.is_empty() {
        return Err(anyhow!("invalid signal"));
    }

    // "fall through" on error as we assume the name is not a number, but
    // a signal name.
    if let Ok(n) = name.parse::<u8>() {
        return Ok(n);
    }

    let mut search_term = if name.starts_with("SIG") {
        name.to_string()
    } else {
        format!("SIG{}", name)
    };

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
    if human_time.is_empty() || human_time.eq("0") {
        return Ok(0);
    }

    let d: humantime::Duration = human_time
        .parse::<humantime::Duration>()
        .map_err(|e| anyhow!(e))?;

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
pub fn get_option(name: &str, options: &mut Options, args: &str) -> Result<String> {
    let words: Vec<&str> = args.split_whitespace().collect();

    for word in words {
        let fields: Vec<String> = word.split('=').map(|s| s.to_string()).collect();

        if fields.len() < 2 {
            continue;
        }

        if fields[0].is_empty() {
            continue;
        }

        let key = fields[0].clone();

        let mut value = fields[1..].join("=");

        // Expand "spec=file:///some/where/config.json"
        if key.eq("spec") && value.starts_with(FILE_URI) {
            let (_, spec_file) = split_uri(&value)?;

            if !spec_file.is_empty() {
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

        return Ok(value.into());
    }

    msg = "generated";

    // Handle option values that can be auto-generated
    let value = match name {
        "cid" => random_container_id(),
        "sid" => random_sandbox_id(),

        // Default to CID
        "exec_id" => {
            msg = "derived";

            options.get("cid").unwrap_or(&"".to_string()).into()
        }
        _ => "".into(),
    };

    debug!(sl!(), "using option {:?}={:?} ({})", name, value, msg);

    // Store auto-generated value
    options.insert(name.to_string(), value.to_string());

    Ok(value)
}

pub fn generate_random_hex_string(len: u32) -> String {
    const CHARSET: &[u8] = b"abcdef0123456789";
    let mut rng = rand::thread_rng();

    let str: String = (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
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
    if bundle_dir.is_empty() {
        debug!(sl!(), "bundle dir is empty");
        return Ok("".to_owned());
    }

    let config_path = PathBuf::from(&bundle_dir).join(CONFIG_FILE);

    config_path
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}", e).context("failed to construct config file path"))
}

fn root_oci_to_ttrpc(bundle_dir: &str, root: &ociRoot) -> Result<ttrpcRoot> {
    let root_dir = root.path().clone().display().to_string();

    let path = if root_dir.starts_with('/') {
        root_dir
    } else {
        // Expand the root directory into an absolute value
        let abs_root_dir = PathBuf::from(&bundle_dir).join(&root_dir);

        abs_root_dir
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("{:?}", e).context("failed to construct bundle path"))?
    };

    let ttrpc_root = ttrpcRoot {
        Path: path,
        Readonly: root.readonly().unwrap_or_default(),
        ..Default::default()
    };

    Ok(ttrpc_root)
}

fn oci_to_ttrpc(bundle_dir: &str, cid: &str, oci: &ociSpec) -> Result<ttrpcSpec> {
    let process = match &oci.process() {
        Some(p) => protobuf::MessageField::some(p.clone().into()),
        None => protobuf::MessageField::none(),
    };

    let root = match &oci.root() {
        Some(r) => {
            let ttrpc_root = root_oci_to_ttrpc(bundle_dir, r)?;

            protobuf::MessageField::some(ttrpc_root)
        }
        None => protobuf::MessageField::none(),
    };

    let mut mounts: Vec<ttrpcMount> = Vec::new();
    let oci_mounts = oci.mounts().clone().unwrap_or_default();
    for m in oci_mounts {
        mounts.push(m.clone().into());
    }

    let linux = match &oci.linux() {
        Some(l) => protobuf::MessageField::some(l.clone().into()),
        None => protobuf::MessageField::none(),
    };

    if cid.len() < MIN_HOSTNAME_LEN as usize {
        return Err(anyhow!("container ID too short for hostname"));
    }

    // FIXME: Implement setting a custom (and unique!) hostname (requires uts ns setup)
    //let hostname = cid[0..MIN_HOSTNAME_LEN as usize].to_string();
    let hostname = "".to_string();

    let ttrpc_spec = ttrpcSpec {
        Version: oci.version().clone(),
        Process: process,
        Root: root,
        Hostname: hostname,
        Mounts: mounts,
        Hooks: protobuf::MessageField::none(),
        Annotations: HashMap::new(),
        Linux: linux,
        Solaris: protobuf::MessageField::none(),
        Windows: protobuf::MessageField::none(),
        ..Default::default()
    };

    Ok(ttrpc_spec)
}

// Split a URI and return a tuple comprising the scheme and the data.
//
// Note that we have to use our own parsing since "json://" is not
// an official schema ;(
fn split_uri(uri: &str) -> Result<(String, String)> {
    const URI_DELIMITER: &str = "://";

    let fields: Vec<&str> = uri.split(URI_DELIMITER).collect();

    if fields.len() != 2 {
        return Err(anyhow!("invalid URI: {:?}", uri));
    }

    Ok((fields[0].into(), fields[1].into()))
}

pub fn spec_file_to_string(spec_file: String) -> Result<String> {
    let oci_spec = ociSpec::load(spec_file).map_err(|e| anyhow!(e))?;

    serde_json::to_string(&oci_spec).map_err(|e| anyhow!(e))
}

pub fn get_oci_spec_json(cfg: &Config) -> Result<String> {
    let spec_file = config_file_from_bundle_dir(&cfg.bundle_dir)?;

    if spec_file.is_empty() {
        debug!(sl!(), "Empty bundle dir");
        return Ok("".to_owned());
    }

    spec_file_to_string(spec_file)
}

pub fn get_ttrpc_spec(options: &mut Options, cid: &str) -> Result<ttrpcSpec> {
    let bundle_dir = get_option("bundle-dir", options, "")?;

    let json_spec = get_option("spec", options, "")?;
    assert_ne!(json_spec, "");

    let oci_spec: ociSpec = serde_json::from_str(&json_spec).map_err(|e| anyhow!(e))?;

    oci_to_ttrpc(&bundle_dir, cid, &oci_spec)
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

// Returns a request object of the type requested.
//
// Call as:
//
// ```rust
// let req1: SomeType = make_request(args)?;
// let req2: AnotherType = make_request(args)?;
// ```
//
// The args string can take a number of forms:
//
// - A file URI:
//
//   The string is expected to start with 'file://' with the full path to
//   a local file containing a complete or partial JSON document.
//
//   Example: 'file:///some/where/foo.json'
//
// - A JSON URI:
//
//   This invented 'json://{ ...}' URI allows either a complete JSON document
//   or a partial JSON fragment to be specified. The JSON takes the form of
//   the JSON serialised protocol buffers files that specify the Kata Agent
//   API.
//
//   - If the complete document for the specified type is provided, the values
//     specified are deserialised into the returned
//     type.
//
//   - If a partial document is provided, the values specified are
//     deserialised into the returned type and all remaining elements take their
//     default values.
//
//   - If no values are specified, all returned type will be created as
//     if TypeName::default() had been specified instead.
//
//   Example 1 (Complete and valid empty JSON document): 'json://{}'
//   Example 2 (Valid partial JSON document): 'json://{"foo": true, "bar": "hello"}'
//   Example 3 (GetGuestDetails API example):
//
//     let args = r#"json://{"mem_block_size": true, "mem_hotplug_probe": true}"#;
//
//     let req: GetGuestDetailsRequest = make_request(args)?;
//
pub fn make_request<T: Default + DeserializeOwned>(args: &str) -> Result<T> {
    if args.is_empty() {
        return Ok(Default::default());
    }

    let (scheme, data) = split_uri(args)?;

    match scheme.as_str() {
        "json" => Ok(serde_json::from_str(&data)?),
        "file" => {
            let file = File::open(data)?;

            Ok(serde_json::from_reader(file)?)
        }
        // Don't error since the args may contain key=value pairs which
        // are not handled by this functionz.
        _ => Ok(Default::default()),
    }
}

pub fn make_copy_file_request(input: &CopyFileInput) -> Result<CopyFileRequest> {
    // create dir mode permissions
    // Dir mode | 750
    let perms = 0o20000000750;

    let src_meta: fs::Metadata = fs::symlink_metadata(&input.src)?;

    let mut req = CopyFileRequest::default();

    req.set_path(input.dest.clone());
    req.set_dir_mode(perms);
    req.set_file_mode(src_meta.mode());
    req.set_uid(src_meta.uid() as i32);
    req.set_gid(src_meta.gid() as i32);
    req.set_offset(0);
    req.set_file_size(0);

    if src_meta.is_symlink() {
        match fs::read_link(&input.src)?.into_os_string().into_string() {
            Ok(path) => {
                req.set_data(path.into_bytes());
            }
            Err(_) => {
                return Err(anyhow!("failed to read link for {}", input.src));
            }
        }
    } else if src_meta.is_file() {
        req.set_file_size(src_meta.size() as i64);
    }

    Ok(req)
}

pub fn make_set_policy_request(input: &SetPolicyInput) -> Result<SetPolicyRequest> {
    let mut policy_file = File::open(&input.policy_file)?;
    let metadata = policy_file.metadata()?;

    let mut policy_data = String::new();
    match policy_file.read_to_string(&mut policy_data) {
        Ok(bytes_read) => {
            if bytes_read != metadata.len() as usize {
                return Err(anyhow!(
                    "Failed to read all policy data, size {} read {}",
                    metadata.len(),
                    bytes_read
                ));
            }
        }
        Err(e) => return Err(anyhow!("Error reading policy file: {}", e)),
    }

    let mut req = SetPolicyRequest::default();
    req.set_policy(policy_data);
    Ok(req)
}

fn fix_oci_process_args(spec: &mut ttrpcSpec, bundle: &str) -> Result<()> {
    let config_path = scoped_join(bundle, CONFIG_FILE)?;

    let file = File::open(config_path)?;
    let oci_from_config: ociSpec = serde_json::from_reader(file)?;

    let mut process: ttrpcProcess = match &oci_from_config.process() {
        Some(p) => p.clone().into(),
        None => {
            return Err(anyhow!("Failed to set container process args"));
        }
    };

    spec.take_Process().set_Args(process.take_Args());
    Ok(())
}

// Helper function to generate create container request
pub fn make_create_container_request(
    input: CreateContainerInput,
) -> Result<CreateContainerRequest> {
    // read in the oci configuration template
    if !Path::new(OCI_CONFIG_TEMPLATE).exists() {
        warn!(sl!(), "make_create_container_request: Missig template file");
        return Err(anyhow!("Missing OCI Config template file"));
    }

    let file = File::open(OCI_CONFIG_TEMPLATE)?;
    let spec: ociSpec = serde_json::from_reader(file)?;

    let mut req = CreateContainerRequest::default();

    let c_id = if !input.id.is_empty() {
        input.id
    } else {
        random_container_id()
    };

    debug!(
        sl!(),
        "make_create_container_request: pulling container image"
    );

    // Pull and unpack the container image
    let bundle = image::pull_image(&input.image, &c_id)?;

    let mut ttrpc_spec = oci_to_ttrpc(&bundle, &c_id, &spec)?;

    // Rootfs has been handled with bundle after pulling image
    // Fix the container process argument.
    fix_oci_process_args(&mut ttrpc_spec, &bundle)?;

    req.set_container_id(c_id);
    req.set_OCI(ttrpc_spec);

    debug!(sl!(), "CreateContainer request generated successfully");

    Ok(req)
}

pub fn remove_container_image_mount(c_id: &str) -> Result<()> {
    image::remove_image_mount(c_id)
}
