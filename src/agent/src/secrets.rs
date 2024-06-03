// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io;
use std::path::Path;
use std::result::Result::Ok;

use anyhow::*;
use kbs_protocol::evidence_provider::NativeEvidenceProvider;
use kbs_protocol::KbsClientBuilder;
use kbs_protocol::KbsClientCapabilities;
use tokio::fs;

use crate::AGENT_CONFIG;

pub const TLS_KEYS_CONFIG_DIR: &str = "/run/tls-keys";
pub const TLS_KEYS_FILE_PATH: &str = "/run/tls-keys/tls-key.zip";
pub const KBS_RESOURCE_PATH: &str = "default/tenant-keys/tls-keys.zip";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// To provision secrets from kbs
pub struct Retriever {
    kbc_name: String,
    kbs_uri: String,
}

impl Retriever {
    // `aa_kbc_params`:  string with format `<kbc_name>::<kbs_uri>`
    pub async fn new(aa_kbc_params: &str) -> Result<Self> {
        if let Some((kbc_name, kbs_uri)) = aa_kbc_params.split_once("::") {
            if kbc_name.is_empty() {
                return Err(anyhow!("aa_kbc_params: missing KBC name"));
            }

            if kbs_uri.is_empty() {
                return Err(anyhow!("aa_kbc_params: missing KBS URI"));
            }

            Ok(Self {
                kbc_name: kbc_name.into(),
                kbs_uri: kbs_uri.into(),
            })
        } else {
            Err(anyhow!("aa_kbc_params: KBC/KBS pair not found"))
        }
    }

    #[allow(dead_code)]
    pub fn extract_zip_file(&mut self) -> Result<()> {
        let fname = std::path::Path::new(TLS_KEYS_FILE_PATH);
        let outdir = std::path::Path::new(TLS_KEYS_CONFIG_DIR);
        let file = std::fs::File::open(fname).unwrap();

        let mut archive = zip::ZipArchive::new(file).unwrap();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let outpath = match file.enclosed_name() {
                Some(path) => outdir.join(path).to_owned(),
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                std::fs::create_dir_all(&outpath).unwrap();
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p).unwrap();
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).unwrap();
                io::copy(&mut file, &mut outfile).unwrap();
            }

            // Get and Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                        .unwrap();
                }
            }
        }
        Ok(())
    }

    pub async fn get_tls_keys(&mut self) -> Result<()> {
        if !Path::new(TLS_KEYS_CONFIG_DIR).exists() {
            fs::create_dir_all(TLS_KEYS_CONFIG_DIR)
                .await
                .map_err(|e| anyhow!("Create tls keys runtime config dir failed: {:?}", e))?;
        }

        info!(
            sl!(),
            "get_tls_key: kbs_name: {} kbc_uri: {} KBS_RESOURCE_PATH: {}",
            &self.kbc_name,
            &self.kbs_uri,
            KBS_RESOURCE_PATH
        );

        // obtain the tls keys from KBS using background check mode
        let kbs_cert = vec![];
        let result =
            get_resource_with_attestation(&self.kbs_uri, KBS_RESOURCE_PATH, None, kbs_cert).await;

        let resource_bytes = match result {
            Ok(data) => data,
            Err(e) => {
                error!(sl!(), " Failed to retrieve get_tls_key: {:?}", e);
                return Err(e);
            }
        };

        fs::write(TLS_KEYS_FILE_PATH, resource_bytes).await?;
        self.extract_zip_file()?;

        Ok(())
    }
}

pub async fn retrieve_secrets() -> Result<()> {
    let aa_kbc_params = &AGENT_CONFIG.aa_kbc_params;

    if !aa_kbc_params.is_empty() {
        let resource_config = format!("provider:attestation-agent:{}", aa_kbc_params);
        if let Some(wrapped_aa_kbc_params) = &Some(&resource_config) {
            let wrapped_aa_kbc_params = wrapped_aa_kbc_params.to_string();
            let m_aa_kbc_params =
                wrapped_aa_kbc_params.trim_start_matches("provider:attestation-agent:");

            let mut retriver = Retriever::new(m_aa_kbc_params).await?;
            retriver.get_tls_keys().await?;
        }
    }
    Ok(())
}

pub fn tls_keys_exist() -> bool {
    // check if the directory of tls keys exists
    if Path::new(TLS_KEYS_CONFIG_DIR).exists() {
        // check if all the necessary tls keys are downloaded and extracted
        if Path::new(TLS_KEYS_CONFIG_DIR).join("server.key").exists()
            && Path::new(TLS_KEYS_CONFIG_DIR).join("server.pem").exists()
            && Path::new(TLS_KEYS_CONFIG_DIR).join("ca.pem").exists()
        {
            return true;
        }
    }

    false
}

pub async fn get_resource_with_attestation(
    url: &str,
    path: &str,
    tee_key_pem: Option<String>,
    kbs_root_certs_pem: Vec<String>,
) -> Result<Vec<u8>> {
    let evidence_provider = Box::new(NativeEvidenceProvider::new()?);
    let mut client_builder = KbsClientBuilder::with_evidence_provider(evidence_provider, url);
    if let Some(key) = tee_key_pem {
        client_builder = client_builder.set_tee_key(&key);
    }

    for cert in kbs_root_certs_pem {
        client_builder = client_builder.add_kbs_cert(&cert)
    }
    let mut client = client_builder.build()?;

    let resource_kbs_uri = format!("kbs:///{path}");
    let resource_bytes = client
        .get_resource(serde_json::from_str(&format!("\"{resource_kbs_uri}\""))?)
        .await?;
    Ok(resource_bytes)
}
