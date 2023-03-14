// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::fmt::{self, Debug};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use crate::config::{DecryptConfig, EncryptConfig, KeyProviderAttrs};
use crate::keywrap::KeyWrapper;
use crate::utils::{self, CommandExecuter};

#[cfg(feature = "keywrap-keyprovider-native")]
use attestation_agent::{AttestationAPIs, AttestationAgent};

#[cfg(feature = "keywrap-keyprovider-native")]
lazy_static! {
    pub static ref ATTESTATION_AGENT: std::sync::Arc<tokio::sync::Mutex<AttestationAgent>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(AttestationAgent::new()));
}

#[derive(Debug)]
enum OpKey {
    Wrap,
    Unwrap,
}

impl fmt::Display for OpKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            OpKey::Wrap => write!(f, "keywrap"),
            OpKey::Unwrap => write!(f, "keyunwrap"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyWrapParams {
    ec: Option<EncryptConfig>,
    #[serde(rename = "optsdata")]
    opts_data: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyUnwrapParams {
    dc: Option<DecryptConfig>,
    annotation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyUnwrapResults {
    #[serde(rename = "optsdata")]
    opts_data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyWrapResults {
    annotation: Vec<u8>,
}

/// KeyProviderKeyWrapProtocolInput defines the input to the key provider binary or grpc method.
#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyProviderKeyWrapProtocolInput {
    /// op is either "keywrap" or "keyunwrap"
    op: String,
    /// keywrapparams encodes the arguments to key wrap if operation is set to wrap
    #[serde(rename = "keywrapparams")]
    key_wrap_params: KeyWrapParams,
    /// keyunwrapparams encodes the arguments to key unwrap if operation is set to unwrap
    #[serde(rename = "keyunwrapparams")]
    key_unwrap_params: KeyUnwrapParams,
}

/// KeyProviderKeyWrapProtocolOutput defines the output of the key provider binary or grpc method.
#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyProviderKeyWrapProtocolOutput {
    /// keywrapresults encodes the results to key wrap if operation is to keywrap
    #[serde(rename = "keywrapresults", skip_serializing_if = "Option::is_none")]
    key_wrap_results: Option<KeyWrapResults>,
    /// keyunwrapresults encodes the result to key unwrap if operation is to keyunwrap
    #[serde(rename = "keyunwrapresults", skip_serializing_if = "Option::is_none")]
    key_unwrap_results: Option<KeyUnwrapResults>,
}

impl KeyProviderKeyWrapProtocolOutput {
    #[cfg(feature = "keywrap-keyprovider-grpc")]
    async fn from_grpc(input: Vec<u8>, conn: &str, operation: OpKey) -> Result<Self> {
        let uri = conn.parse::<tonic::codegen::http::Uri>().unwrap();
        // create a channel ie connection to server
        let channel = tonic::transport::Channel::builder(uri)
            .connect()
            .await
            .map_err(|e| anyhow!("keyprovider: error while creating channel: {e}"))?;

        let mut client =
            crate::utils::grpc::keyprovider::key_provider_service_client::KeyProviderServiceClient::new(
                channel,
            );
        let msg = crate::utils::grpc::keyprovider::KeyProviderKeyWrapProtocolInput {
            key_provider_key_wrap_protocol_input: input,
        };
        let request = tonic::Request::new(msg);
        let grpc_output = match operation {
            OpKey::Wrap => client.wrap_key(request).await.map_err(|e| {
                anyhow!(
                    "keyprovider: error from grpc server for {} operation, {}",
                    OpKey::Wrap,
                    e
                )
            })?,

            OpKey::Unwrap => client.un_wrap_key(request).await.map_err(|e| {
                anyhow!(
                    "keyprovider: error from grpc server for {} operation, {}",
                    OpKey::Unwrap,
                    e
                )
            })?,
        };

        serde_json::from_slice(
            &grpc_output
                .into_inner()
                .key_provider_key_wrap_protocol_output,
        )
        .map_err(|_| {
            anyhow!(
                "Error while deserializing grpc output on {} operation",
                OpKey::Unwrap
            )
        })
    }

    #[cfg(feature = "keywrap-keyprovider-ttrpc")]
    async fn from_ttrpc(input: Vec<u8>, conn: &str, operation: OpKey) -> Result<Self> {
        let c = ttrpc::r#async::Client::connect(conn)?;

        let kc = crate::utils::ttrpc::keyprovider_ttrpc::KeyProviderServiceClient::new(c);
        let kc1 = kc.clone();
        let mut req = crate::utils::ttrpc::keyprovider::KeyProviderKeyWrapProtocolInput::new();
        req.KeyProviderKeyWrapProtocolInput = input;

        let ttrpc_output = match operation {
            OpKey::Wrap => kc1
                .wrap_key(ttrpc::context::with_timeout(20 * 1000 * 1000), &req)
                .await
                .map_err(|_| {
                    anyhow!(
                        "keyprovider: Error from ttrpc server for {:?} operation",
                        OpKey::Wrap.to_string()
                    )
                })?,

            OpKey::Unwrap => kc1
                .un_wrap_key(ttrpc::context::with_timeout(20 * 1000 * 1000), &req)
                .await
                .map_err(|_| {
                    anyhow!(
                        "keyprovider: Error from ttrpc server for {:?} operation",
                        OpKey::Unwrap.to_string()
                    )
                })?,
        };

        serde_json::from_slice(&ttrpc_output.KeyProviderKeyWrapProtocolOutput).map_err(|_| {
            anyhow!(
                "Error while deserializing ttrpc output on {:?} operation",
                OpKey::Unwrap.to_string()
            )
        })
    }

    #[cfg(feature = "keywrap-keyprovider-cmd")]
    fn from_command(
        input: Vec<u8>,
        command: &crate::config::Command,
        runner: &dyn crate::utils::CommandExecuter,
    ) -> Result<Self> {
        let cmd_name = command.path.to_string();
        let default_args = vec![];
        let args = command.args.as_ref().unwrap_or(&default_args);
        let resp_bytes: Vec<u8> = runner
            .exec(cmd_name, args, input)
            .map_err(|e| anyhow!("keyprovider: error from command executor: {}", e))?;

        serde_json::from_slice(&resp_bytes)
            .map_err(|_| anyhow!("keyprovider: failed to deserialize message from binary executor"))
    }

    #[cfg(feature = "keywrap-keyprovider-native")]
    fn from_native(annotation: &str, dc_config: &DecryptConfig) -> Result<Self> {
        let kbc_kbs_pair = if let Some(list) = dc_config.param.get("attestation-agent") {
            list.get(0)
                .ok_or_else(|| anyhow!("keyprovider: empty kbc::kbs pair"))?
        } else {
            return Err(anyhow!("keyprovider: not supported attestation agent"));
        };
        let pair_str = String::from_utf8(kbc_kbs_pair.to_vec())?;
        let (kbc, kbs) = pair_str
            .split_once("::")
            .ok_or_else(|| anyhow!("keyprovider: invalid kbc::kbs pair"))?;
        let kbc = kbc.to_string();
        let kbs = kbs.to_string();
        let annotation = annotation.to_string();

        let handler = std::thread::spawn(move || {
            create_async_runtime()?.block_on(async {
                ATTESTATION_AGENT
                    .lock()
                    .await
                    .decrypt_image_layer_annotation(&kbc, &kbs, &annotation)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        });

        match handler.join() {
            Ok(Ok(v)) => Ok(KeyProviderKeyWrapProtocolOutput {
                key_unwrap_results: Some(KeyUnwrapResults { opts_data: v }),
                ..Default::default()
            }),
            Ok(Err(e)) => Err(anyhow!("keyprovider: retrieve opts_data failed: {e}")),
            Err(e) => Err(anyhow!("keyprovider: retrieve opts_data failed: {e:?}")),
        }
    }
}

/// A KeyProvider keywrapper
#[derive(Debug)]
pub struct KeyProviderKeyWrapper {
    pub provider: String,
    pub attrs: KeyProviderAttrs,
    pub runner: Option<Box<dyn CommandExecuter>>,
}

impl KeyProviderKeyWrapper {
    /// Create a new instance of `KeyProviderKeyWrapper`.
    pub fn new(
        provider: String,
        mut attrs: KeyProviderAttrs,
        runner: Option<Box<dyn utils::CommandExecuter>>,
    ) -> Self {
        if let Some(grpc) = &attrs.grpc {
            if !grpc.starts_with("http://") && !grpc.starts_with("tcp://") {
                attrs.grpc = Some(format!("http://{grpc}"));
            }
        }

        KeyProviderKeyWrapper {
            provider,
            attrs,
            runner,
        }
    }

    fn wrap_key_cmd(&self, _input: Vec<u8>, _cmd: &crate::config::Command) -> Result<Vec<u8>> {
        if let Some(_runner) = self.runner.as_ref() {
            #[cfg(not(feature = "keywrap-keyprovider-cmd"))]
            {
                Err(anyhow!("keyprovider: no support of keyprovider-grpc"))
            }
            #[cfg(feature = "keywrap-keyprovider-cmd")]
            {
                let protocol_output = KeyProviderKeyWrapProtocolOutput::from_command(
                    _input, _cmd, _runner,
                )
                .map_err(|e| {
                    anyhow!(
                        "keyprovider: error from binary provider for {} operation: {e}",
                        OpKey::Wrap,
                    )
                })?;
                if let Some(result) = protocol_output.key_wrap_results {
                    Ok(result.annotation)
                } else {
                    Err(anyhow!("keyprovider: get NULL reply from provider"))
                }
            }
        } else {
            Err(anyhow!("keyprovider: runner for binary provider is NULL"))
        }
    }

    fn wrap_key_grpc(&self, _input: Vec<u8>, grpc: &str) -> Result<Vec<u8>> {
        #[cfg(not(feature = "keywrap-keyprovider-grpc"))]
        {
            Err(anyhow!(
                "keyprovider: no support of keyprovider-grpc, {}",
                grpc
            ))
        }
        #[cfg(feature = "keywrap-keyprovider-grpc")]
        {
            let grpc = grpc.to_string();
            let handler = std::thread::spawn(move || {
                create_async_runtime()?.block_on(async {
                    KeyProviderKeyWrapProtocolOutput::from_grpc(_input, &grpc, OpKey::Wrap)
                        .await
                        .map_err(|e| format!("{e}"))
                })
            });
            let protocol_output = match handler.join() {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    return Err(anyhow!(
                        "keyprovider: grpc provider failed to execute {} operation: {}",
                        OpKey::Wrap,
                        e
                    ));
                }
                Err(e) => {
                    return Err(anyhow!(
                        "keyprovider: grpc provider failed to execute {} operation: {e:?}",
                        OpKey::Wrap,
                    ));
                }
            };
            if let Some(result) = protocol_output.key_wrap_results {
                Ok(result.annotation)
            } else {
                Err(anyhow!("keyprovider: get NULL reply from provider"))
            }
        }
    }

    fn wrap_key_ttrpc(&self, _input: Vec<u8>, ttrpc: &str) -> Result<Vec<u8>> {
        #[cfg(not(feature = "keywrap-keyprovider-ttrpc"))]
        {
            Err(anyhow!(
                "keyprovider: no support of keyprovider-ttrpc, {}",
                ttrpc
            ))
        }
        #[cfg(feature = "keywrap-keyprovider-ttrpc")]
        {
            let ttrpc = ttrpc.to_string();
            let handler = std::thread::spawn(move || {
                create_async_runtime()?.block_on(async {
                    KeyProviderKeyWrapProtocolOutput::from_ttrpc(_input, &ttrpc, OpKey::Wrap)
                        .await
                        .map_err(|e| format!("{e}"))
                })
            });
            let protocol_output = match handler.join() {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    return Err(anyhow!(
                        "keyprovider: ttrpc provider failed to execute {} operation: {}",
                        OpKey::Wrap,
                        e
                    ));
                }
                Err(e) => {
                    return Err(anyhow!(
                        "keyprovider: ttrpc provider failed to execute {} operation: {e:?}",
                        OpKey::Wrap,
                    ));
                }
            };
            if let Some(result) = protocol_output.key_wrap_results {
                Ok(result.annotation)
            } else {
                Err(anyhow!("keyprovider: get NULL reply from provider"))
            }
        }
    }

    fn unwrap_key_cmd(
        &self,
        _input: Vec<u8>,
        _cmd: &crate::config::Command,
    ) -> Result<KeyProviderKeyWrapProtocolOutput> {
        if let Some(_runner) = self.runner.as_ref() {
            #[cfg(not(feature = "keywrap-keyprovider-cmd"))]
            return Err(anyhow!("keyprovider: no support of keyprovider-grpc"));
            #[cfg(feature = "keywrap-keyprovider-cmd")]
            {
                KeyProviderKeyWrapProtocolOutput::from_command(_input, _cmd, _runner).map_err(|e| {
                    anyhow!(
                        "keyprovider: error from binary provider for {} operation: {e}",
                        OpKey::Unwrap,
                    )
                })
            }
        } else {
            bail!("keyprovider: runner for binary provider is NULL");
        }
    }

    fn unwrap_key_grpc(
        &self,
        _input: Vec<u8>,
        grpc: &str,
    ) -> Result<KeyProviderKeyWrapProtocolOutput> {
        #[cfg(not(feature = "keywrap-keyprovider-grpc"))]
        return Err(anyhow!(
            "keyprovider: no support of keyprovider-grpc, {}",
            grpc
        ));
        #[cfg(feature = "keywrap-keyprovider-grpc")]
        {
            let grpc = grpc.to_string();
            let handler = std::thread::spawn(move || {
                create_async_runtime()?.block_on(async {
                    KeyProviderKeyWrapProtocolOutput::from_grpc(_input, &grpc, OpKey::Unwrap)
                        .await
                        .map_err(|e| {
                            format!(
                                "keyprovider: grpc provider failed to execute {} operation: {e}",
                                OpKey::Wrap,
                            )
                        })
                })
            });
            match handler.join() {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(e)) => bail!("failed to unwrap key by gRPC, {e}"),
                Err(e) => bail!("failed to unwrap key by gRPC, {e:?}"),
            }
        }
    }

    fn unwrap_key_ttrpc(
        &self,
        _input: Vec<u8>,
        ttrpc: &str,
    ) -> Result<KeyProviderKeyWrapProtocolOutput> {
        #[cfg(not(feature = "keywrap-keyprovider-ttrpc"))]
        return Err(anyhow!(
            "keyprovider: no support of keyprovider-ttrpc, {}",
            ttrpc
        ));
        #[cfg(feature = "keywrap-keyprovider-ttrpc")]
        {
            let ttrpc = ttrpc.to_string();
            let handler = std::thread::spawn(move || {
                create_async_runtime()?.block_on(async {
                    KeyProviderKeyWrapProtocolOutput::from_ttrpc(_input, &ttrpc, OpKey::Unwrap)
                        .await
                        .map_err(|e| {
                            format!(
                                "keyprovider: ttrpc provider failed to execute {} operation: {e}",
                                OpKey::Wrap,
                            )
                        })
                })
            });
            match handler.join() {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(e)) => bail!("failed to unwrap key by gRPC, {e}"),
                Err(e) => bail!("failed to unwrap key by gRPC, {e:?}"),
            }
        }
    }

    fn unwrap_key_native(
        &self,
        _dc_config: &DecryptConfig,
        _json_string: &[u8],
    ) -> Result<KeyProviderKeyWrapProtocolOutput> {
        #[cfg(not(feature = "keywrap-keyprovider-native"))]
        return Err(anyhow!("keyprovider: no support of keyprovider-native"));
        #[cfg(feature = "keywrap-keyprovider-native")]
        {
            let content = String::from_utf8(_json_string.to_vec())?;
            KeyProviderKeyWrapProtocolOutput::from_native(&content, _dc_config).map_err(|e| {
                anyhow!(
                    "keyprovider: error from crate provider for {} operation: {e}",
                    OpKey::Unwrap,
                )
            })
        }
    }
}

impl KeyWrapper for KeyProviderKeyWrapper {
    /// WrapKeys calls appropriate binary-executable or grpc/ttrpc server for wrapping the session
    /// key for recipients and gets encrypted optsData, which describe the symmetric key used for
    /// encrypting the layer.
    fn wrap_keys(&self, enc_config: &EncryptConfig, opts_data: &[u8]) -> Result<Vec<u8>> {
        if !enc_config.param.contains_key(&self.provider) {
            return Err(anyhow!(
                "keyprovider: unknown provider {} for operation {}",
                &self.provider,
                OpKey::Wrap
            ));
        }

        let opts_data_str = String::from_utf8(opts_data.to_vec())
            .map_err(|_| anyhow!("keyprovider: can not convert option data to string"))?;
        let key_wrap_params = KeyWrapParams {
            ec: Some(enc_config.clone()),
            opts_data: Some(opts_data_str),
        };
        let input = KeyProviderKeyWrapProtocolInput {
            op: OpKey::Wrap.to_string(),
            key_wrap_params,
            key_unwrap_params: KeyUnwrapParams::default(),
        };
        let _serialized_input = serde_json::to_vec(&input).map_err(|_| {
            anyhow!(
                "keyprovider: error while serializing input parameters for {} operation",
                OpKey::Wrap
            )
        })?;

        if let Some(_cmd) = &self.attrs.cmd {
            self.wrap_key_cmd(_serialized_input, _cmd)
        } else if let Some(grpc) = self.attrs.grpc.as_ref() {
            self.wrap_key_grpc(_serialized_input, grpc)
        } else if let Some(ttrpc) = self.attrs.ttrpc.as_ref() {
            self.wrap_key_ttrpc(_serialized_input, ttrpc)
        } else {
            Err(anyhow!(
                "keyprovider: invalid configuration, both grpc and runner are NULL"
            ))
        }
    }

    /// UnwrapKey calls appropriate binary-executable or grpc/ttrpc server for unwrapping the
    /// session key based on the protocol given in annotation for recipients and gets decrypted
    /// optsData, which describe the symmetric key used for decrypting the layer
    fn unwrap_keys(&self, dc_config: &DecryptConfig, json_string: &[u8]) -> Result<Vec<u8>> {
        let annotation_str = String::from_utf8(json_string.to_vec())
            .map_err(|_| anyhow!("keyprovider: can not convert json data to string"))?;
        let key_unwrap_params = KeyUnwrapParams {
            dc: Some(dc_config.clone()),
            annotation: Some(base64::encode(annotation_str)),
        };
        let input = KeyProviderKeyWrapProtocolInput {
            op: OpKey::Unwrap.to_string(),
            key_wrap_params: KeyWrapParams::default(),
            key_unwrap_params,
        };
        let _serialized_input = serde_json::to_vec(&input).map_err(|_| {
            anyhow!(
                "keyprovider: error while serializing input parameters for {} operation",
                OpKey::Unwrap
            )
        })?;

        let _protocol_output = if let Some(cmd) = self.attrs.cmd.as_ref() {
            self.unwrap_key_cmd(_serialized_input, cmd)?
        } else if let Some(grpc) = self.attrs.grpc.as_ref() {
            self.unwrap_key_grpc(_serialized_input, grpc)?
        } else if let Some(ttrpc) = self.attrs.ttrpc.as_ref() {
            self.unwrap_key_ttrpc(_serialized_input, ttrpc)?
        } else if let Some(_native) = self.attrs.native.as_ref() {
            self.unwrap_key_native(dc_config, json_string)?
        } else {
            return Err(anyhow!(
                "keyprovider: invalid configuration, both grpc and runner are NULL"
            ));
        };

        if let Some(result) = _protocol_output.key_unwrap_results {
            Ok(result.opts_data)
        } else {
            Err(anyhow!("keyprovider: get NULL reply from provider"))
        }
    }

    fn annotation_id(&self) -> String {
        format!(
            "org.opencontainers.image.enc.keys.provider.{}",
            self.provider
        )
    }

    fn probe(&self, _dc_param: &HashMap<String, Vec<Vec<u8>>>) -> bool {
        true
    }
}

#[cfg(any(
    feature = "keywrap-keyprovider-grpc",
    feature = "keywrap-keyprovider-ttrpc",
    feature = "keywrap-keyprovider-native"
))]
fn create_async_runtime() -> std::result::Result<tokio::runtime::Runtime, String> {
    match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Err(e) => Err(format!("keyprovider: failed to create async runtime, {e}")),
        Ok(rt) => Ok(rt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "keywrap-keyprovider-native")]
    use crate::helpers::create_decrypt_config;

    ///Test runner which mocks binary executable for key wrapping and unwrapping
    #[derive(Clone, Copy)]
    pub struct TestRunner {}

    ///Mock annotation packet, which goes into container image manifest
    #[derive(Serialize, Deserialize, Debug)]
    struct AnnotationPacket {
        pub key_url: String,
        pub wrapped_key: Vec<u8>,
        pub wrap_type: String,
    }

    ///grpc server with mock api implementation for serving the clients with mock WrapKey and Unwrapkey grpc method implementations
    #[derive(Default)]
    struct TestServer {}

    #[cfg(any(
        feature = "keywrap-keyprovider-cmd",
        feature = "keywrap-keyprovider-grpc",
        feature = "keywrap-keyprovider-ttrpc"
    ))]
    mod cmd_grpc {
        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::aes::{Aes256Dec, Aes256Enc};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use anyhow::{anyhow, Result};

        pub static mut ENC_KEY: &[u8; 32] = b"passphrasewhichneedstobe32bytes!";
        pub static mut DEC_KEY: &[u8; 32] = b"passphrasewhichneedstobe32bytes!";

        pub fn encrypt_key(plain_text: &[u8], encrypting_key: &[u8; 32]) -> Result<Vec<u8>> {
            let encrypting_key = Key::<Aes256Enc>::from_slice(encrypting_key);
            let cipher = Aes256Gcm::new(encrypting_key);
            let nonce = Nonce::from_slice(b"unique nonce");

            cipher
                .encrypt(nonce, plain_text.as_ref())
                .map_err(|_| anyhow!("encryption failure"))
        }

        pub fn decrypt_key(cipher_text: &[u8], decrypting_key: &[u8; 32]) -> Result<Vec<u8>> {
            let decrypting_key = Key::<Aes256Dec>::from_slice(decrypting_key);
            let cipher = Aes256Gcm::new(decrypting_key);
            let nonce = Nonce::from_slice(b"unique nonce");

            cipher
                .decrypt(nonce, cipher_text.as_ref())
                .map_err(|_| anyhow!("decryption failure"))
        }
    }

    #[cfg(feature = "keywrap-keyprovider-grpc")]
    mod grpc {
        use super::cmd_grpc::{decrypt_key, encrypt_key, DEC_KEY, ENC_KEY};
        use super::*;
        use crate::utils::grpc::keyprovider::key_provider_service_server::KeyProviderService;
        use crate::utils::grpc::keyprovider::key_provider_service_server::KeyProviderServiceServer;
        use crate::utils::grpc::keyprovider::{
            KeyProviderKeyWrapProtocolInput as grpc_input,
            KeyProviderKeyWrapProtocolOutput as grpc_output,
        };
        use std::net::SocketAddr;
        use tokio::sync::mpsc;
        use tonic;
        use tonic::{transport::Server, Request};

        #[tonic::async_trait]
        impl KeyProviderService for TestServer {
            async fn wrap_key(
                &self,
                request: Request<grpc_input>,
            ) -> core::result::Result<tonic::Response<grpc_output>, tonic::Status> {
                let key_wrap_input: super::super::KeyProviderKeyWrapProtocolInput =
                    serde_json::from_slice(
                        &request.into_inner().key_provider_key_wrap_protocol_input,
                    )
                    .unwrap();
                let plain_optsdata = key_wrap_input.key_wrap_params.opts_data.unwrap();
                if let Ok(wrapped_key_result) =
                    encrypt_key(&base64::decode(plain_optsdata).unwrap(), unsafe { ENC_KEY })
                {
                    let ap = AnnotationPacket {
                        key_url: "https://key-provider/key-uuid".to_string(),
                        wrapped_key: wrapped_key_result,
                        wrap_type: "AES".to_string(),
                    };
                    let serialized_ap = serde_json::to_vec(&ap).unwrap();
                    let key_wrap_output = super::super::KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: Some(super::super::KeyWrapResults {
                            annotation: serialized_ap,
                        }),
                        key_unwrap_results: None,
                    };
                    let serialized_key_wrap_output = serde_json::to_vec(&key_wrap_output).unwrap();

                    Ok(tonic::Response::new(grpc_output {
                        key_provider_key_wrap_protocol_output: serialized_key_wrap_output,
                    }))
                } else {
                    Err(tonic::Status::unknown("Error while encrypting key"))
                }
            }

            async fn un_wrap_key(
                &self,
                request: Request<grpc_input>,
            ) -> core::result::Result<tonic::Response<grpc_output>, tonic::Status> {
                let key_wrap_input: super::super::KeyProviderKeyWrapProtocolInput =
                    serde_json::from_slice(
                        &request.into_inner().key_provider_key_wrap_protocol_input,
                    )
                    .unwrap();
                let base64_annotation = key_wrap_input.key_unwrap_params.annotation.unwrap();
                let vec_annotation = base64::decode(base64_annotation).unwrap();
                let str_annotation: &str = std::str::from_utf8(&vec_annotation).unwrap();
                let annotation_packet: AnnotationPacket =
                    serde_json::from_str(str_annotation).unwrap();
                let wrapped_key = annotation_packet.wrapped_key;
                if let Ok(unwrapped_key_result) = decrypt_key(&wrapped_key, unsafe { DEC_KEY }) {
                    let key_wrap_output = super::super::KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: None,
                        key_unwrap_results: Some(super::super::KeyUnwrapResults {
                            opts_data: unwrapped_key_result,
                        }),
                    };
                    let serialized_key_wrap_output = serde_json::to_vec(&key_wrap_output).unwrap();
                    Ok(tonic::Response::new(grpc_output {
                        key_provider_key_wrap_protocol_output: serialized_key_wrap_output,
                    }))
                } else {
                    Err(tonic::Status::unknown("Error while decrypting key"))
                }
            }
        }

        // Function to start a mock grpc server
        pub fn start_grpc_server(sock_address: String) {
            tokio::spawn(async move {
                let (tx, mut rx) = mpsc::unbounded_channel();
                let addr: SocketAddr = sock_address.parse().unwrap();
                let server = TestServer::default();
                let serve = Server::builder()
                    .add_service(KeyProviderServiceServer::new(server))
                    .serve(addr);

                tokio::spawn(async move {
                    if let Err(e) = serve.await {
                        eprintln!("Error = {e}");
                    }

                    tx.send(()).unwrap();
                });

                rx.recv().await;
            });
        }
    }

    #[cfg(feature = "keywrap-keyprovider-ttrpc")]
    mod ttrpc_test {
        use super::cmd_grpc::{decrypt_key, encrypt_key, DEC_KEY, ENC_KEY};
        use super::*;
        use crate::utils::ttrpc::keyprovider::{
            KeyProviderKeyWrapProtocolInput as ttrpc_input,
            KeyProviderKeyWrapProtocolOutput as ttrpc_output,
        };
        use crate::utils::ttrpc::keyprovider_ttrpc;
        use async_trait::async_trait;
        use std::fs;
        use std::path::Path;
        use tokio::signal::unix::{signal, SignalKind};
        pub const SOCK_ADDR: &str = "unix:///tmp/ttrpc-test";

        #[async_trait]
        impl keyprovider_ttrpc::KeyProviderService for TestServer {
            async fn wrap_key(
                &self,
                _ctx: &::ttrpc::r#async::TtrpcContext,
                req: ttrpc_input,
            ) -> ::ttrpc::Result<ttrpc_output> {
                let key_wrap_input: super::super::KeyProviderKeyWrapProtocolInput =
                    serde_json::from_slice(&req.KeyProviderKeyWrapProtocolInput).unwrap();
                let plain_optsdata = key_wrap_input.key_wrap_params.opts_data.unwrap();
                if let Ok(wrapped_key_result) =
                    encrypt_key(&base64::decode(plain_optsdata).unwrap(), unsafe { ENC_KEY })
                {
                    let ap = AnnotationPacket {
                        key_url: "https://key-provider/key-uuid".to_string(),
                        wrapped_key: wrapped_key_result,
                        wrap_type: "AES".to_string(),
                    };
                    let serialized_ap = serde_json::to_vec(&ap).unwrap();
                    let key_wrap_output = KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: Some(KeyWrapResults {
                            annotation: serialized_ap,
                        }),
                        key_unwrap_results: None,
                    };
                    let serialized_key_wrap_output = serde_json::to_vec(&key_wrap_output).unwrap();
                    let mut key_ttrpc_result = ttrpc_output::new();
                    key_ttrpc_result.KeyProviderKeyWrapProtocolOutput = serialized_key_wrap_output;
                    Ok(key_ttrpc_result)
                } else {
                    let mut status = ttrpc::Status::new();
                    status.set_code(ttrpc::Code::NOT_FOUND);
                    Err(ttrpc::error::Error::RpcStatus(status))
                }
            }

            async fn un_wrap_key(
                &self,
                _ctx: &::ttrpc::r#async::TtrpcContext,
                req: ttrpc_input,
            ) -> ::ttrpc::Result<ttrpc_output> {
                let key_wrap_input: super::super::KeyProviderKeyWrapProtocolInput =
                    serde_json::from_slice(&req.KeyProviderKeyWrapProtocolInput).unwrap();

                let base64_annotation = key_wrap_input.key_unwrap_params.annotation.unwrap();
                let vec_annotation = base64::decode(base64_annotation).unwrap();
                let str_annotation: &str = std::str::from_utf8(&vec_annotation).unwrap();
                let annotation_packet: AnnotationPacket =
                    serde_json::from_str(str_annotation).unwrap();
                let wrapped_key = annotation_packet.wrapped_key;
                if let Ok(unwrapped_key_result) = decrypt_key(&wrapped_key, unsafe { DEC_KEY }) {
                    let key_wrap_output = KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: None,
                        key_unwrap_results: Some(KeyUnwrapResults {
                            opts_data: unwrapped_key_result,
                        }),
                    };
                    let serialized_key_wrap_output = serde_json::to_vec(&key_wrap_output).unwrap();
                    let mut key_ttrpc_result = ttrpc_output::new();
                    key_ttrpc_result.KeyProviderKeyWrapProtocolOutput = serialized_key_wrap_output;
                    Ok(key_ttrpc_result)
                } else {
                    let mut status = ttrpc::Status::new();
                    status.set_code(ttrpc::Code::NOT_FOUND);
                    Err(ttrpc::error::Error::RpcStatus(status))
                }
            }
        }

        fn remove_if_sock_exist(sock_addr: &str) -> std::io::Result<()> {
            let path = sock_addr
                .strip_prefix("unix://")
                .expect("socket address is not expected");

            if Path::new(path).exists() {
                fs::remove_file(path)?;
            }

            Ok(())
        }

        // Run a mock ttrpc server
        pub fn start_ttrpc_server() {
            tokio::spawn(async move {
                let k = Box::<crate::keywrap::keyprovider::tests::TestServer>::default()
                    as Box<dyn keyprovider_ttrpc::KeyProviderService + Send + Sync>;
                let kp_service = keyprovider_ttrpc::create_key_provider_service(k.into());

                remove_if_sock_exist(SOCK_ADDR).unwrap();

                let mut server = ttrpc::asynchronous::Server::new()
                    .bind(SOCK_ADDR)
                    .unwrap()
                    .register_service(kp_service);

                let mut interrupt = signal(SignalKind::interrupt()).unwrap();
                server.start().await.unwrap();
                tokio::select! {
                    _ = interrupt.recv() => {
                        // test graceful shutdown
                        println!("graceful shutdown");
                        server.shutdown().await.unwrap();
                    }
                };
            });
        }
    }

    #[cfg(feature = "keywrap-keyprovider-cmd")]
    mod cmd {
        use super::super::{
            KeyProviderKeyWrapProtocolInput, KeyProviderKeyWrapProtocolOutput, KeyUnwrapResults,
            KeyWrapResults,
        };
        use super::*;

        impl crate::utils::CommandExecuter for TestRunner {
            /// Mock CommandExecuter for executing a linux command line command and return the output of the command with an error if it exists.
            fn exec(
                &self,
                cmd: String,
                _args: &[std::string::String],
                input: Vec<u8>,
            ) -> anyhow::Result<Vec<u8>> {
                let mut key_wrap_output = KeyProviderKeyWrapProtocolOutput::default();
                if cmd == "/usr/lib/keyprovider-wrapkey" {
                    let key_wrap_input: KeyProviderKeyWrapProtocolInput =
                        serde_json::from_slice(input.as_ref()).unwrap();
                    let plain_optsdata = key_wrap_input.key_wrap_params.opts_data.unwrap();
                    let wrapped_key = self::cmd_grpc::encrypt_key(
                        &base64::decode(plain_optsdata).unwrap(),
                        unsafe { self::cmd_grpc::ENC_KEY },
                    )
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                    let ap = AnnotationPacket {
                        key_url: "https://key-provider/key-uuid".to_string(),
                        wrapped_key,
                        wrap_type: "AES".to_string(),
                    };
                    let serialized_ap = serde_json::to_vec(&ap).unwrap();
                    key_wrap_output = KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: Some(KeyWrapResults {
                            annotation: serialized_ap,
                        }),
                        key_unwrap_results: None,
                    };
                } else if cmd == "/usr/lib/keyprovider-unwrapkey" {
                    let key_wrap_input: KeyProviderKeyWrapProtocolInput =
                        serde_json::from_slice(input.as_ref()).unwrap();
                    let base64_annotation = key_wrap_input.key_unwrap_params.annotation.unwrap();
                    let vec_annotation = base64::decode(base64_annotation).unwrap();
                    let str_annotation: &str = std::str::from_utf8(&vec_annotation).unwrap();
                    let annotation_packet: AnnotationPacket =
                        serde_json::from_str(str_annotation).unwrap();
                    let wrapped_key = annotation_packet.wrapped_key;
                    let unwrapped_key = super::cmd_grpc::decrypt_key(&wrapped_key, unsafe {
                        super::cmd_grpc::DEC_KEY
                    })
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                    key_wrap_output = KeyProviderKeyWrapProtocolOutput {
                        key_wrap_results: None,
                        key_unwrap_results: Some(KeyUnwrapResults {
                            opts_data: unwrapped_key,
                        }),
                    };
                }
                let serialized_keywrap_output = serde_json::to_vec(&key_wrap_output).unwrap();

                Ok(serialized_keywrap_output)
            }
        }
    }

    #[cfg(feature = "keywrap-keyprovider-cmd")]
    #[test]
    #[ignore]
    fn test_key_provider_command_success() {
        let test_runner = TestRunner {};
        let mut provider = std::collections::HashMap::new();
        let mut dc_params = vec![];
        let mut attrs = crate::config::KeyProviderAttrs {
            cmd: Some(crate::config::Command {
                path: "/usr/lib/keyprovider-wrapkey".to_string(),
                args: None,
            }),
            grpc: None,
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let mut keyprovider_key_wrapper = KeyProviderKeyWrapper::new(
            "keyprovider".to_string(),
            attrs.clone(),
            Some(Box::new(test_runner)),
        );

        unsafe {
            self::cmd_grpc::ENC_KEY = b"passphrasewhichneedstobe32bytes!";
            self::cmd_grpc::DEC_KEY = b"passphrasewhichneedstobe32bytes!";
        }

        // Prepare for mock encryption config
        let opts_data = b"symmetric_key";
        let b64_opts_data = base64::encode(opts_data).into_bytes();
        let mut ec = crate::keywrap::EncryptConfig::default();
        let mut dc = crate::keywrap::DecryptConfig::default();
        let mut ec_params = vec![];
        let param = "keyprovider".to_string().into_bytes();
        ec_params.push(param.clone());
        assert!(ec.encrypt_with_key_provider(ec_params).is_ok());
        assert!(keyprovider_key_wrapper
            .wrap_keys(&ec, &b64_opts_data)
            .is_ok());

        // Perform key-provider wrap-key operation
        let key_wrap_output_result = keyprovider_key_wrapper.wrap_keys(&ec, &b64_opts_data);

        // Create keyprovider-key-wrapper
        attrs = crate::config::KeyProviderAttrs {
            cmd: Some(crate::config::Command {
                path: "/usr/lib/keyprovider-unwrapkey".to_string(),
                args: None,
            }),
            grpc: None,
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        keyprovider_key_wrapper = KeyProviderKeyWrapper::new(
            "keyprovider".to_string(),
            attrs,
            Some(Box::new(test_runner)),
        );
        // Prepare for mock encryption config
        dc_params.push(param);
        assert!(dc.decrypt_with_key_provider(dc_params).is_ok());
        let json_string = key_wrap_output_result.unwrap();
        // Perform key-provider wrap-key operation
        let key_wrap_output_result = keyprovider_key_wrapper.unwrap_keys(&dc, &json_string);
        let unwrapped_key = key_wrap_output_result.unwrap();
        assert_eq!(opts_data.to_vec(), unwrapped_key);
    }

    #[cfg(feature = "keywrap-keyprovider-cmd")]
    #[test]
    fn test_command_executer_wrap_key_fail() {
        let test_runner = TestRunner {};
        let mut ec_params = vec![];
        let mut provider = std::collections::HashMap::new();
        let attrs = crate::config::KeyProviderAttrs {
            cmd: Some(crate::config::Command {
                path: "/usr/lib/keyprovider-wrapkey".to_string(),
                args: None,
            }),
            grpc: None,
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper = KeyProviderKeyWrapper::new(
            "keyprovider".to_string(),
            attrs,
            Some(Box::new(test_runner)),
        );

        let b64_opts_data = base64::encode(b"symmetric_key").into_bytes();
        let mut ec = crate::keywrap::EncryptConfig::default();
        ec_params.push("keyprovider1".to_string().into_bytes());
        assert!(ec.encrypt_with_key_provider(ec_params).is_ok());
        assert!(keyprovider_key_wrapper
            .wrap_keys(&ec, &b64_opts_data)
            .is_err());
    }

    #[cfg(feature = "keywrap-keyprovider-cmd")]
    #[test]
    fn test_command_executer_unwrap_key_fail() {
        let test_runner = TestRunner {};
        let mut dc_params = vec![];

        let mut provider = std::collections::HashMap::new();
        let attrs = crate::config::KeyProviderAttrs {
            cmd: Some(crate::config::Command {
                path: "/usr/lib/keyprovider-unwrapkey".to_string(),
                args: None,
            }),
            grpc: None,
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper = KeyProviderKeyWrapper::new(
            "keyprovider".to_string(),
            attrs,
            Some(Box::new(test_runner)),
        );

        // Perform manual encryption
        let opts_data = b"symmetric_key";
        let wrapped_key =
            self::cmd_grpc::encrypt_key(opts_data.as_ref(), unsafe { self::cmd_grpc::ENC_KEY })
                .unwrap();
        let ap = AnnotationPacket {
            key_url: "https://key-provider/key-uuid".to_string(),
            wrapped_key,
            wrap_type: "AES".to_string(),
        };
        let serialized_ap = serde_json::to_vec(&ap).unwrap();

        // Change the decryption key so that decryption should fail
        unsafe { self::cmd_grpc::DEC_KEY = b"wrong_passwhichneedstobe32bytes!" };

        // Prepare for mock decryption config
        dc_params.push("keyprovider1".to_string().as_bytes().to_vec());
        let mut dc = crate::keywrap::DecryptConfig::default();
        assert!(dc.decrypt_with_key_provider(dc_params).is_ok());
        assert!(keyprovider_key_wrapper
            .unwrap_keys(&dc, &serialized_ap)
            .is_err());
    }

    #[cfg(feature = "keywrap-keyprovider-grpc")]
    #[test]
    fn test_key_provider_grpc_tcp_success() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        self::grpc::start_grpc_server("127.0.0.1:8990".to_string());
        // sleep for few seconds so that grpc server bootstraps
        std::thread::sleep(std::time::Duration::from_secs(1));
        unsafe {
            self::cmd_grpc::ENC_KEY = b"passphrasewhichneedstobe32bytes!";
            self::cmd_grpc::DEC_KEY = b"passphrasewhichneedstobe32bytes!";
        }

        let mut provider = std::collections::HashMap::new();
        let mut dc_params = vec![];
        let attrs = crate::config::KeyProviderAttrs {
            cmd: None,
            grpc: Some("tcp://127.0.0.1:8990".to_string()),
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper =
            KeyProviderKeyWrapper::new("keyprovider".to_string(), attrs, None);

        // Prepare encryption config params
        let opts_data = b"symmetric_key";
        let b64_opts_data = base64::encode(opts_data).into_bytes();
        let mut ec = crate::keywrap::EncryptConfig::default();
        let mut dc = crate::keywrap::DecryptConfig::default();
        let mut ec_params = vec![];
        let param = "keyprovider".to_string().into_bytes();
        ec_params.push(param.clone());
        assert!(ec.encrypt_with_key_provider(ec_params).is_ok());
        let key_wrap_output_result = keyprovider_key_wrapper.wrap_keys(&ec, &b64_opts_data);

        // Perform decryption-config params
        dc_params.push(param);
        assert!(dc.decrypt_with_key_provider(dc_params).is_ok());
        let json_string = key_wrap_output_result.unwrap();

        // Perform unwrapkey operation
        let key_wrap_output_result = keyprovider_key_wrapper.unwrap_keys(&dc, &json_string);
        let unwrapped_key = key_wrap_output_result.unwrap();
        assert_eq!(opts_data.to_vec(), unwrapped_key);
        // runtime shutdown for stopping grpc server
        rt.shutdown_background();
    }

    #[cfg(feature = "keywrap-keyprovider-grpc")]
    #[test]
    fn test_key_provider_grpc_http_success() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        self::grpc::start_grpc_server("127.0.0.1:8991".to_string());
        // sleep for few seconds so that grpc server bootstraps
        std::thread::sleep(std::time::Duration::from_secs(1));
        unsafe {
            self::cmd_grpc::ENC_KEY = b"passphrasewhichneedstobe32bytes!";
            self::cmd_grpc::DEC_KEY = b"passphrasewhichneedstobe32bytes!";
        }

        let mut provider = std::collections::HashMap::new();
        let mut dc_params = vec![];
        let attrs = crate::config::KeyProviderAttrs {
            cmd: None,
            grpc: Some("http://127.0.0.1:8991".to_string()),
            ttrpc: None,
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper =
            KeyProviderKeyWrapper::new("keyprovider".to_string(), attrs, None);

        // Prepare encryption config params
        let opts_data = b"symmetric_key";
        let b64_opts_data = base64::encode(opts_data).into_bytes();
        let mut ec = crate::keywrap::EncryptConfig::default();
        let mut dc = crate::keywrap::DecryptConfig::default();
        let mut ec_params = vec![];
        let param = "keyprovider".to_string().into_bytes();
        ec_params.push(param.clone());
        assert!(ec.encrypt_with_key_provider(ec_params).is_ok());
        let key_wrap_output_result = keyprovider_key_wrapper.wrap_keys(&ec, &b64_opts_data);

        // Perform decryption-config params
        dc_params.push(param);
        assert!(dc.decrypt_with_key_provider(dc_params).is_ok());
        let json_string = key_wrap_output_result.unwrap();

        // Perform unwrapkey operation
        let key_wrap_output_result = keyprovider_key_wrapper.unwrap_keys(&dc, &json_string);
        let unwrapped_key = key_wrap_output_result.unwrap();
        assert_eq!(opts_data.to_vec(), unwrapped_key);

        // runtime shutdown for stopping grpc server
        rt.shutdown_background();
    }

    #[cfg(feature = "keywrap-keyprovider-ttrpc")]
    #[test]
    fn test_key_provider_ttrpc_success() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        self::ttrpc_test::start_ttrpc_server();
        std::thread::sleep(std::time::Duration::from_secs(2));
        unsafe {
            self::cmd_grpc::ENC_KEY = b"passphrasewhichneedstobe32bytes!";
            self::cmd_grpc::DEC_KEY = b"passphrasewhichneedstobe32bytes!";
        }

        let mut provider = HashMap::new();
        let mut dc_params = vec![];
        let attrs = crate::config::KeyProviderAttrs {
            cmd: None,
            grpc: None,
            ttrpc: Some(self::ttrpc_test::SOCK_ADDR.to_string()),
            native: None,
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper =
            KeyProviderKeyWrapper::new("keyprovider".to_string(), attrs, None);

        // Prepare encryption config params
        let opts_data = b"symmetric_key";
        let b64_opts_data = base64::encode(opts_data).into_bytes();
        let mut ec = EncryptConfig::default();
        let mut dc = DecryptConfig::default();
        let mut ec_params = vec![];
        let param = "keyprovider".to_string().into_bytes();
        ec_params.push(param.clone());
        assert!(ec.encrypt_with_key_provider(ec_params).is_ok());
        let key_wrap_output_result = keyprovider_key_wrapper.wrap_keys(&ec, &b64_opts_data);

        // Perform decryption-config params
        dc_params.push(param);
        assert!(dc.decrypt_with_key_provider(dc_params).is_ok());
        let json_string = key_wrap_output_result.unwrap();

        // Perform unwrapkey operation
        let key_wrap_output_result = keyprovider_key_wrapper.unwrap_keys(&dc, &json_string);
        let unwrapped_key = key_wrap_output_result.unwrap();
        assert_eq!(opts_data.to_vec(), unwrapped_key);
        // runtime shutdown for stopping ttrpc server
        rt.shutdown_background();
    }

    #[cfg(feature = "keywrap-keyprovider-native")]
    #[test]
    fn test_key_provider_native_fail() {
        let dummy_annotation: &str = "{}";
        let mut provider = std::collections::HashMap::new();
        let attrs = crate::config::KeyProviderAttrs {
            cmd: None,
            grpc: None,
            ttrpc: None,
            native: Some("attestation-agent".to_string()),
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper =
            KeyProviderKeyWrapper::new("attestation-agent".to_string(), attrs, None);

        let unsupported_aa_parameters: &str = "provider:unsupported-aa:sample_kbc::null";
        let unsupported_cc =
            create_decrypt_config(vec![unsupported_aa_parameters.to_string()], vec![]).unwrap();
        let unsupported_dc = unsupported_cc.decrypt_config.unwrap();
        let unsupported_res =
            keyprovider_key_wrapper.unwrap_keys(&unsupported_dc, dummy_annotation.as_bytes());
        assert!(unsupported_res.is_err());
        let unsupported_msg = format!("{}", unsupported_res.unwrap_err());
        assert!(unsupported_msg.contains("keyprovider: not supported attestation agent"));

        let invalid_pair_aa_parameters: &str = "provider:attestation-agent:*";
        let invalid_pair_cc =
            create_decrypt_config(vec![invalid_pair_aa_parameters.to_string()], vec![]).unwrap();
        let invalid_pair_dc = invalid_pair_cc.decrypt_config.unwrap();
        let invalid_pair_res =
            keyprovider_key_wrapper.unwrap_keys(&invalid_pair_dc, dummy_annotation.as_bytes());
        assert!(invalid_pair_res.is_err());
        let invalid_pair_msg = format!("{}", invalid_pair_res.unwrap_err());
        assert!(invalid_pair_msg.contains("keyprovider: invalid kbc::kbs pair"));
    }

    #[cfg(feature = "keywrap-keyprovider-native")]
    #[test]
    fn test_key_provider_native_succuss() {
        let annotation_from_sample_kbc: Vec<u8> = {
            let res = std::fs::read("data/sample_kbc_annotation.json");
            if let Ok(out) = res {
                out
            } else {
                vec![]
            }
        };

        let mut provider = std::collections::HashMap::new();
        let attrs = crate::config::KeyProviderAttrs {
            cmd: None,
            grpc: None,
            ttrpc: None,
            native: Some("attestation-agent".to_string()),
        };
        provider.insert(String::from("provider"), attrs.clone());
        let keyprovider_key_wrapper =
            KeyProviderKeyWrapper::new("attestation-agent".to_string(), attrs, None);

        let aa_parameters: &str = "provider:attestation-agent:sample_kbc::null";
        let cc = create_decrypt_config(vec![aa_parameters.to_string()], vec![]).unwrap();
        let dc = cc.decrypt_config.unwrap();
        let res = keyprovider_key_wrapper.unwrap_keys(&dc, &annotation_from_sample_kbc);
        assert!(res.is_ok());
    }
}
