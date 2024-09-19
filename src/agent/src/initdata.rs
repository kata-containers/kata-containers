// Copyright (c) 2024 Alibaba
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    env,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use const_format::concatcp;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha384, Sha512};
use slog::Logger;
use tokio::{fs, process::Command};

use crate::{cdh::CDHClient, config::GuestComponentsProcs, AGENT_CONFIG};

/// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// Unix socket prefix
const UNIX_SOCKET_PREFIX: &str = "unix://";

/// Now only initdata `0.1.0` is defined.
const DEFAULT_INITDATA_VERSION: &str = "0.1.0";

/// Timeout to launch guest components
const DEFAULT_LAUNCH_PROCESS_TIMEOUT: i32 = 6;

/// Path of AA binary
const AA_PATH: &str = "/usr/local/bin/attestation-agent";

/// Path to AA unix socket.
const AA_ATTESTATION_SOCKET: &str =
    "/run/confidential-containers/attestation-agent/attestation-agent.sock";

/// Path of AA config, which is writable inside TEE.
const AA_CONFIG_PATH: &str = "/run/aa.toml";

/// Binary path of CDH
const CDH_PATH: &str = "/usr/local/bin/confidential-data-hub";

/// Path to CDH unix socket.
const CDH_SOCKET: &str = "/run/confidential-containers/cdh.sock";

/// Unix socket of CDH
pub const CDH_SOCKET_URI: &str = concatcp!(UNIX_SOCKET_PREFIX, CDH_SOCKET);

/// Path of CDH config, which is writable inside TEE.
const CDH_CONFIG_PATH: &str = "/run/cdh.toml";

/// Path of ocicrypt config file. This is used by image-rs when decrypting image.
const OCICRYPT_CONFIG_PATH: &str = "/tmp/ocicrypt_config.json";

/// Path to ASR binary
const API_SERVER_PATH: &str = "/usr/local/bin/api-server-rest";

/// If no initdata is given, a default (all zero) initdata will be used.
const DEFAULT_INITDATA_DIGEST: &str = "AAAAAAAAAAAA";

/// Initdata defined in
/// <https://github.com/confidential-containers/trustee/blob/47d7a2338e0be76308ac19be5c0c172c592780aa/kbs/docs/initdata.md>
#[derive(Deserialize)]
pub struct Initdata {
    version: String,
    algorithm: String,
    data: DefinedFields,
}

static ALREADY_SET_INITDATA: AtomicBool = AtomicBool::new(false);

/// Well-defined keys for initdata of kata/CoCo
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DefinedFields {
    #[serde(rename = "aa.toml")]
    aa_config: Option<String>,
    #[serde(rename = "cdh.toml")]
    cdh_config: Option<String>,
    #[serde(rename = "policy.rego")]
    policy: Option<String>,
}

pub async fn do_set_initdata(req: &protocols::agent::SetInitdataRequest) -> Result<()> {
    debug!(sl!(), "SetInitdata called.");
    // ensure the function could be called only once.
    if ALREADY_SET_INITDATA
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        debug!(sl!(), "SetInitdata called the first time.");
    } else {
        bail!("SetInitdata already called");
    }

    let (initdata_digest, configs) = if !req.initdata.is_empty() {
        let initdata = base64::engine::general_purpose::STANDARD
            .decode(&req.initdata)
            .context("illegal base64 encoded initdata")?;
        let initdata: Initdata =
            toml::from_slice(&initdata).context("parse initdata TOML failed")?;
        debug!(sl!(), "Initdata parse done.");

        if initdata.version != DEFAULT_INITDATA_VERSION {
            bail!("Unsupported initdata version, should be {DEFAULT_INITDATA_VERSION}");
        }

        let digest = match &initdata.algorithm[..] {
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(req.initdata.as_bytes());
                hasher.finalize().to_vec()
            }
            "sha384" => {
                let mut hasher = Sha384::new();
                hasher.update(req.initdata.as_bytes());
                hasher.finalize().to_vec()
            }
            "sha512" => {
                let mut hasher = Sha512::new();
                hasher.update(req.initdata.as_bytes());
                hasher.finalize().to_vec()
            }
            others => bail!("Unsupported hash algorithm {others}"),
        };

        let initdata_digest = base64::engine::general_purpose::STANDARD.encode(digest);
        (initdata_digest, initdata.data)
    } else {
        debug!(
            sl!(),
            "SetInitdata called with empty initdata. Launch guest components with default configs"
        );
        (DEFAULT_INITDATA_DIGEST.into(), DefinedFields::default())
    };

    init_attestation_components(
        &sl!(),
        configs.aa_config,
        configs.cdh_config,
        initdata_digest,
    )
    .await
    .context("launch guest components failed")?;

    if let Some(_policy) = configs.policy {
        debug!(sl!(), "Agent policy is given, try to set policy.");

        #[cfg(feature = "agent-policy")]
        {
            let mut policy_agent = crate::AGENT_POLICY.lock().await;
            // SetInitData will allow a one-time SetPolicy. By default policy allow to set, thus we
            // do not need to check if API is allowed.
            policy_agent
                .set_policy(&_policy)
                .await
                .context("set policy failed")?;

            debug!(sl!(), "Set policy successfully.");
        }

        #[cfg(not(feature = "agent-policy"))]
        debug!(sl!(), "Feature `agent-policy` is not enabled in kata-agent, ignore the policy field in initdata");
    }

    Ok(())
}

async fn wait_for_path_to_exist(logger: &Logger, path: &str, timeout_secs: i32) -> Result<()> {
    let p = Path::new(path);
    let mut attempts = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if p.exists() {
            return Ok(());
        }
        if attempts >= timeout_secs {
            break;
        }
        attempts += 1;
        info!(
            logger,
            "waiting for {} to exist (attempts={})", path, attempts
        );
    }

    Err(anyhow!("wait for {} to exist timeout.", path))
}

pub async fn launch_process(
    logger: &Logger,
    path: &str,
    args: Vec<String>,
    unix_socket_path: &str,
    timeout_secs: i32,
) -> Result<()> {
    if !Path::new(path).exists() {
        bail!("path {} does not exist.", path);
    }
    if !unix_socket_path.is_empty() && Path::new(unix_socket_path).exists() {
        fs::remove_file(unix_socket_path).await?;
    }
    Command::new(path).args(args).spawn()?;
    if !unix_socket_path.is_empty() && timeout_secs > 0 {
        wait_for_path_to_exist(logger, unix_socket_path, timeout_secs).await?;
    }

    Ok(())
}

async fn launch_confidential_data_hub(cdh_config: Option<String>) -> Result<()> {
    if Path::new(CDH_PATH).exists() {
        let args = match cdh_config {
            Some(config) => {
                debug!(
                    sl!(),
                    "Launch Confidential Data Hub with config delivered via Initdata"
                );
                tokio::fs::write(CDH_CONFIG_PATH, config.as_bytes()).await?;
                vec!["-c".into(), CDH_CONFIG_PATH.into()]
            }
            None => {
                debug!(sl!(), "Launch Confidential Data Hub with default config");
                vec![]
            }
        };

        let ocicrypt_config = serde_json::json!({
            "key-providers": {
                "attestation-agent":{
                    "ttrpc":CDH_SOCKET_URI
                }
            }
        });

        fs::write(OCICRYPT_CONFIG_PATH, ocicrypt_config.to_string().as_bytes()).await?;
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", OCICRYPT_CONFIG_PATH);

        launch_process(
            &sl!(),
            CDH_PATH,
            args,
            CDH_SOCKET,
            DEFAULT_LAUNCH_PROCESS_TIMEOUT,
        )
        .await?;
        debug!(
            sl!(),
            "Confidential Data Hub has been launched successfully."
        );
    }

    Ok(())
}

async fn launch_attestation_agent(aa_config: Option<String>, initdata: String) -> Result<()> {
    if Path::new(AA_PATH).exists() {
        let mut args = match aa_config {
            Some(config) => {
                debug!(
                    sl!(),
                    "Launch Attestation Agent with config delivered via Initdata"
                );
                tokio::fs::write(AA_CONFIG_PATH, config.as_bytes()).await?;
                vec!["-c".into(), AA_CONFIG_PATH.into()]
            }
            None => {
                debug!(sl!(), "Launch Attestation Agent with default config");
                vec![]
            }
        };

        args.push("--initdata".into());
        args.push(initdata);

        launch_process(
            &sl!(),
            CDH_PATH,
            args,
            AA_ATTESTATION_SOCKET,
            DEFAULT_LAUNCH_PROCESS_TIMEOUT,
        )
        .await?;
        debug!(sl!(), "Attestation Agent has been launched successfully.");
    }

    Ok(())
}

// Check if required attestation binaries are available on the rootfs.
fn attestation_binaries_available(logger: &Logger, procs: &GuestComponentsProcs) -> bool {
    let binaries = match procs {
        GuestComponentsProcs::AttestationAgent => vec![AA_PATH],
        GuestComponentsProcs::ConfidentialDataHub => vec![AA_PATH, CDH_PATH],
        GuestComponentsProcs::ApiServerRest => vec![AA_PATH, CDH_PATH, API_SERVER_PATH],
        _ => vec![],
    };
    for binary in binaries.iter() {
        if !Path::new(binary).exists() {
            warn!(logger, "{} not found", binary);
            return false;
        }
    }
    true
}

// Start-up attestation-agent, CDH and api-server-rest if they are packaged in the rootfs
// and the corresponding procs are enabled in the agent configuration. the process will be
// launched in the background and the function will return immediately.
// If the CDH is started, a CDH client will be instantiated and returned.
async fn init_attestation_components(
    logger: &Logger,
    aa_config: Option<String>,
    cdh_config: Option<String>,
    initdata_digest: String,
) -> Result<()> {
    let agent_config = &AGENT_CONFIG;

    let gc_procs = agent_config.guest_components_procs;
    if gc_procs != GuestComponentsProcs::None {
        if !attestation_binaries_available(logger, &gc_procs) {
            warn!(
                logger,
                "attestation binaries requested for launch not available"
            );
        }
    }

    // skip launch of any guest-component
    if agent_config.guest_components_procs == GuestComponentsProcs::None {
        return Ok(());
    }

    debug!(logger, "spawning attestation-agent process {}", AA_PATH);
    launch_attestation_agent(aa_config, initdata_digest)
        .await
        .map_err(|e| anyhow!("launch_process {} failed: {:?}", AA_PATH, e))?;

    // skip launch of confidential-data-hub and api-server-rest
    if agent_config.guest_components_procs == GuestComponentsProcs::AttestationAgent {
        return Ok(());
    }

    debug!(
        logger,
        "spawning confidential-data-hub process {}", CDH_PATH
    );
    launch_confidential_data_hub(cdh_config)
        .await
        .map_err(|e| anyhow!("launch_process {} failed: {:?}", CDH_PATH, e))?;

    let cdh_client = CDHClient::new().context("Failed to create CDH Client")?;

    crate::cdh::CDH_CLIENT
        .set(cdh_client)
        .context("fail to initialize CDH client")?;
    // skip launch of api-server-rest
    if agent_config.guest_components_procs == GuestComponentsProcs::ConfidentialDataHub {
        return Ok(());
    }

    let features = agent_config.guest_components_rest_api;
    debug!(
        logger,
        "spawning api-server-rest process {} --features {}", API_SERVER_PATH, features
    );

    launch_process(
        logger,
        API_SERVER_PATH,
        vec!["--features".into(), features.to_string()],
        "",
        0,
    )
    .await
    .map_err(|e| anyhow!("launch_process {} failed: {:?}", API_SERVER_PATH, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Initdata;

    #[rstest]
    #[case(
        r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"attestation-agent.conf" = '''
{
"kbs_addr": "http://172.18.0.1:8080"
}
'''
"#,
        true
    )]
    #[case(
        r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"attestation-agent.conf" = """
[token_configs]

[token_configs.coco_as]
url = "http://127.0.0.1:8000"

[token_configs.kbs]
url = "https://127.0.0.1:8080"
cert = '''
-----BEGIN CERTIFICATE-----
MIIDljCCAn6gAwIBAgIUR/UNh13GFam4emgludtype/S9BIwDQYJKoZIhvcNAQEL
BQAwdTELMAkGA1UEBhMCQ04xETAPBgNVBAgMCFpoZWppYW5nMREwDwYDVQQHDAhI
YW5nemhvdTERMA8GA1UECgwIQUFTLVRFU1QxFDASBgNVBAsMC0RldmVsb3BtZW50
MRcwFQYDVQQDDA5BQVMtVEVTVC1IVFRQUzAeFw0yNDAzMTgwNzAzNTNaFw0yNTAz
MTgwNzAzNTNaMHUxCzAJBgNVBAYTAkNOMREwDwYDVQQIDAhaaGVqaWFuZzERMA8G
A1UEBwwISGFuZ3pob3UxETAPBgNVBAoMCEFBUy1URVNUMRQwEgYDVQQLDAtEZXZl
bG9wbWVudDEXMBUGA1UEAwwOQUFTLVRFU1QtSFRUUFMwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQDfp1aBr6LiNRBlJUcDGcAbcUCPG6UzywtVIc8+comS
ay//gwz2AkDmFVvqwI4bdp/NUCwSC6ShHzxsrCEiagRKtA3af/ckM7hOkb4S6u/5
ewHHFcL6YOUp+NOH5/dSLrFHLjet0dt4LkyNBPe7mKAyCJXfiX3wb25wIBB0Tfa0
p5VoKzwWeDQBx7aX8TKbG6/FZIiOXGZdl24DGARiqE3XifX7DH9iVZ2V2RL9+3WY
05GETNFPKtcrNwTy8St8/HsWVxjAzGFzf75Lbys9Ff3JMDsg9zQzgcJJzYWisxlY
g3CmnbENP0eoHS4WjQlTUyY0mtnOwodo4Vdf8ZOkU4wJAgMBAAGjHjAcMBoGA1Ud
EQQTMBGCCWxvY2FsaG9zdIcEfwAAATANBgkqhkiG9w0BAQsFAAOCAQEAKW32spii
t2JB7C1IvYpJw5mQ5bhIlldE0iB5rwWvNbuDgPrgfTI4xiX5sumdHw+P2+GU9KXF
nWkFRZ9W/26xFrVgGIS/a07aI7xrlp0Oj+1uO91UhCL3HhME/0tPC6z1iaFeZp8Y
T1tLnafqiGiThFUgvg6PKt86enX60vGaTY7sslRlgbDr9sAi/NDSS7U1PviuC6yo
yJi7BDiRSx7KrMGLscQ+AKKo2RF1MLzlJMa1kIZfvKDBXFzRd61K5IjDRQ4HQhwX
DYEbQvoZIkUTc1gBUWDcAUS5ztbJg9LCb9WVtvUTqTP2lGuNymOvdsuXq+sAZh9b
M9QaC1mzQ/OStg==
-----END CERTIFICATE-----
'''
"""

"policy.rego" = '''
package agent_policy

import future.keywords.in
import future.keywords.every

import input

# Default values, returned by OPA when rules cannot be evaluated to true.
default CopyFileRequest := false
default CreateContainerRequest := false
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default ExecProcessRequest := false
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default OnlineCPUMemRequest := true
default PullImageRequest := true
default ReadStreamRequest := false
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StatsContainerRequest := true
default TtyWinResizeRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := false'''
"#,
        true
    )]
    #[case(
        r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"attestation-agent.conf" = """
[token_configs]

[token_configs.coco_as]
url = "http://127.0.0.1:8000"

[token_configs.kbs]
url = "https://127.0.0.1:8080"
cert = '''
-----BEGIN CERTIFICATE-----
MIIDljCCAn6gAwIBAgIUR/UNh13GFam4emgludtype/S9BIwDQYJKoZIhvcNAQEL
BQAwdTELMAkGA1UEBhMCQ04xETAPBgNVBAgMCFpoZWppYW5nMREwDwYDVQQHDAhI
YW5nemhvdTERMA8GA1UECgwIQUFTLVRFU1QxFDASBgNVBAsMC0RldmVsb3BtZW50
MRcwFQYDVQQDDA5BQVMtVEVTVC1IVFRQUzAeFw0yNDAzMTgwNzAzNTNaFw0yNTAz
MTgwNzAzNTNaMHUxCzAJBgNVBAYTAkNOMREwDwYDVQQIDAhaaGVqaWFuZzERMA8G
A1UEBwwISGFuZ3pob3UxETAPBgNVBAoMCEFBUy1URVNUMRQwEgYDVQQLDAtEZXZl
bG9wbWVudDEXMBUGA1UEAwwOQUFTLVRFU1QtSFRUUFMwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQDfp1aBr6LiNRBlJUcDGcAbcUCPG6UzywtVIc8+comS
ay//gwz2AkDmFVvqwI4bdp/NUCwSC6ShHzxsrCEiagRKtA3af/ckM7hOkb4S6u/5
ewHHFcL6YOUp+NOH5/dSLrFHLjet0dt4LkyNBPe7mKAyCJXfiX3wb25wIBB0Tfa0
p5VoKzwWeDQBx7aX8TKbG6/FZIiOXGZdl24DGARiqE3XifX7DH9iVZ2V2RL9+3WY
05GETNFPKtcrNwTy8St8/HsWVxjAzGFzf75Lbys9Ff3JMDsg9zQzgcJJzYWisxlY
g3CmnbENP0eoHS4WjQlTUyY0mtnOwodo4Vdf8ZOkU4wJAgMBAAGjHjAcMBoGA1Ud
EQQTMBGCCWxvY2FsaG9zdIcEfwAAATANBgkqhkiG9w0BAQsFAAOCAQEAKW32spii
t2JB7C1IvYpJw5mQ5bhIlldE0iB5rwWvNbuDgPrgfTI4xiX5sumdHw+P2+GU9KXF
nWkFRZ9W/26xFrVgGIS/a07aI7xrlp0Oj+1uO91UhCL3HhME/0tPC6z1iaFeZp8Y
T1tLnafqiGiThFUgvg6PKt86enX60vGaTY7sslRlgbDr9sAi/NDSS7U1PviuC6yo
yJi7BDiRSx7KrMGLscQ+AKKo2RF1MLzlJMa1kIZfvKDBXFzRd61K5IjDRQ4HQhwX
DYEbQvoZIkUTc1gBUWDcAUS5ztbJg9LCb9WVtvUTqTP2lGuNymOvdsuXq+sAZh9b
M9QaC1mzQ/OStg==
-----END CERTIFICATE-----
'''
"""

"policy.rego" = '''
package agent_policy

import future.keywords.in
import future.keywords.every

import input

# Default values, returned by OPA when rules cannot be evaluated to true.
default CopyFileRequest := false
default CreateContainerRequest := false
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default ExecProcessRequest := false
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default OnlineCPUMemRequest := true
default PullImageRequest := true
default ReadStreamRequest := false
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StatsContainerRequest := true
default TtyWinResizeRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := false'''
"confidential-data-hub.conf" = '''
# The ttrpc sock of CDH that is used to listen to the requests
socket = "unix:///run/confidential-containers/cdh.sock"

# KBC related configs.
[kbc]
# Required. The KBC name. It could be `cc_kbc`, `online_sev_kbc` or
# `offline_fs_kbc`. All the items under `[credentials]` will be
# retrieved using the kbc.
name = "cc_kbc"

# Required. The URL of KBS. If `name` is either `cc_kbc` or
# `online_sev_kbc`, this URL will be used to connect to the
# CoCoKBS (for cc_kbc) or Simple-KBS (for online_sev_kbc). If
# `name` is `offline_fs_kbc`, This URL will be ignored.
url = "http://example.io:8080"

# Optional. The public key cert of KBS. If not given, CDH will
# try to use HTTP to connect the server.
kbs_cert = """
-----BEGIN CERTIFICATE-----
MIIFTDCCAvugAwIBAgIBADBGBgkqhkiG9w0BAQowOaAPMA0GCWCGSAFlAwQCAgUA
oRwwGgYJKoZIhvcNAQEIMA0GCWCGSAFlAwQCAgUAogMCATCjAwIBATB7MRQwEgYD
VQQLDAtFbmdpbmVlcmluZzELMAkGA1UEBhMCVVMxFDASBgNVBAcMC1NhbnRhIENs
YXJhMQswCQYDVQQIDAJDQTEfMB0GA1UECgwWQWR2YW5jZWQgTWljcm8gRGV2aWNl
czESMBAGA1UEAwwJU0VWLU1pbGFuMB4XDTIzMDEyNDE3NTgyNloXDTMwMDEyNDE3
NTgyNlowejEUMBIGA1UECwwLRW5naW5lZXJpbmcxCzAJBgNVBAYTAlVTMRQwEgYD
VQQHDAtTYW50YSBDbGFyYTELMAkGA1UECAwCQ0ExHzAdBgNVBAoMFkFkdmFuY2Vk
IE1pY3JvIERldmljZXMxETAPBgNVBAMMCFNFVi1WQ0VLMHYwEAYHKoZIzj0CAQYF
K4EEACIDYgAExmG1ZbuoAQK93USRyZQcsyobfbaAEoKEELf/jK39cOVJt1t4s83W
XM3rqIbS7qHUHQw/FGyOvdaEUs5+wwxpCWfDnmJMAQ+ctgZqgDEKh1NqlOuuKcKq
2YAWE5cTH7sHo4IBFjCCARIwEAYJKwYBBAGceAEBBAMCAQAwFwYJKwYBBAGceAEC
BAoWCE1pbGFuLUIwMBEGCisGAQQBnHgBAwEEAwIBAzARBgorBgEEAZx4AQMCBAMC
AQAwEQYKKwYBBAGceAEDBAQDAgEAMBEGCisGAQQBnHgBAwUEAwIBADARBgorBgEE
AZx4AQMGBAMCAQAwEQYKKwYBBAGceAEDBwQDAgEAMBEGCisGAQQBnHgBAwMEAwIB
CDARBgorBgEEAZx4AQMIBAMCAXMwTQYJKwYBBAGceAEEBEDDhCejDUx6+dlvehW5
cmmCWmTLdqI1L/1dGBFdia1HP46MC82aXZKGYSutSq37RCYgWjueT+qCMBE1oXDk
d1JOMEYGCSqGSIb3DQEBCjA5oA8wDQYJYIZIAWUDBAICBQChHDAaBgkqhkiG9w0B
AQgwDQYJYIZIAWUDBAICBQCiAwIBMKMDAgEBA4ICAQACgCai9x8DAWzX/2IelNWm
ituEBSiq9C9eDnBEckQYikAhPasfagnoWFAtKu/ZWTKHi+BMbhKwswBS8W0G1ywi
cUWGlzigI4tdxxf1YBJyCoTSNssSbKmIh5jemBfrvIBo1yEd+e56ZJMdhN8e+xWU
bvovUC2/7Dl76fzAaACLSorZUv5XPJwKXwEOHo7FIcREjoZn+fKjJTnmdXce0LD6
9RHr+r+ceyE79gmK31bI9DYiJoL4LeGdXZ3gMOVDR1OnDos5lOBcV+quJ6JujpgH
d9g3Sa7Du7pusD9Fdap98ocZslRfFjFi//2YdVM4MKbq6IwpYNB+2PCEKNC7SfbO
NgZYJuPZnM/wViES/cP7MZNJ1KUKBI9yh6TmlSsZZOclGJvrOsBZimTXpATjdNMt
cluKwqAUUzYQmU7bf2TMdOXyA9iH5wIpj1kWGE1VuFADTKILkTc6LzLzOWCofLxf
onhTtSDtzIv/uel547GZqq+rVRvmIieEuEvDETwuookfV6qu3D/9KuSr9xiznmEg
xynud/f525jppJMcD/ofbQxUZuGKvb3f3zy+aLxqidoX7gca2Xd9jyUy5Y/83+ZN
bz4PZx81UJzXVI9ABEh8/xilATh1ZxOePTBJjN7lgr0lXtKYjV/43yyxgUYrXNZS
oLSG2dLCK9mjjraPjau34Q==
-----END CERTIFICATE-----
"""

# credentials are items that will be retrieved from KBS when CDH
# is launched. `resource_uri` refers to the KBS resource uri and
# `path` is where to place the file.
# `path` must be with prefix `/run/confidential-containers/cdh`,
# or it will be blocked by CDH.
[[credentials]]
path = "/run/confidential-containers/cdh/kms-credential/aliyun/ecsRamRole.json"
resource_uri = "kbs:///default/aliyun/ecs_ram_role"

[[credentials]]
path = "/run/confidential-containers/cdh/test/file"
resource_uri = "kbs:///default/test/file"
'''
"#,
        true
    )]
    #[case(
        r#"
algorithm = "sha384"
version = "0.1.0"

[data]
"malwared-field" = "some-value"
"#,
        false
    )]
    fn parse_init_data(#[case] data: &str, #[case] ok: bool) {
        let res = toml::from_str::<Initdata>(data);
        assert_eq!(res.is_ok(), ok);
    }
}
