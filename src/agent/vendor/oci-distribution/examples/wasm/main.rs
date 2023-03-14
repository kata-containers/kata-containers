use oci_distribution::{annotations, secrets::RegistryAuth, Client, Reference};

use docker_credential::{CredentialRetrievalError, DockerCredential};
use std::collections::HashMap;
use tracing::{debug, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

mod cli;
use clap::Parser;
use cli::Cli;

mod pull;
use pull::pull_wasm;

mod push;
use push::push_wasm;

fn build_auth(reference: &Reference, cli: &Cli) -> RegistryAuth {
    let server = reference
        .resolve_registry()
        .strip_suffix("/")
        .unwrap_or_else(|| reference.resolve_registry());

    if cli.anonymous {
        return RegistryAuth::Anonymous;
    }

    match docker_credential::get_credential(server) {
        Err(CredentialRetrievalError::ConfigNotFound) => RegistryAuth::Anonymous,
        Err(CredentialRetrievalError::NoCredentialConfigured) => RegistryAuth::Anonymous,
        Err(e) => panic!("Error handling docker configuration file: {}", e),
        Ok(DockerCredential::UsernamePassword(username, password)) => {
            debug!("Found docker credentials");
            RegistryAuth::Basic(username, password)
        }
        Ok(DockerCredential::IdentityToken(_)) => {
            warn!("Cannot use contents of docker config, identity token not supported. Using anonymous auth");
            RegistryAuth::Anonymous
        }
    }
}

fn build_client_config(cli: &Cli) -> oci_distribution::client::ClientConfig {
    let protocol = if cli.insecure {
        oci_distribution::client::ClientProtocol::Http
    } else {
        oci_distribution::client::ClientProtocol::Https
    };

    oci_distribution::client::ClientConfig {
        protocol,
        ..Default::default()
    }
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();

    // setup logging
    let level_filter = if cli.verbose { "debug" } else { "info" };
    let filter_layer = EnvFilter::new(level_filter);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    let client_config = build_client_config(&cli);
    let mut client = Client::new(client_config);

    match &cli.command {
        crate::cli::Commands::Pull { output, image } => {
            let reference: Reference = image.parse().expect("Not a valid image reference");
            let auth = build_auth(&reference, &cli);
            pull_wasm(&mut client, &auth, &reference, &output).await;
        }
        crate::cli::Commands::Push {
            module,
            image,
            annotations,
        } => {
            let reference: Reference = image.parse().expect("Not a valid image reference");
            let auth = build_auth(&reference, &cli);

            let annotations = if annotations.is_empty() {
                let mut values: HashMap<String, String> = HashMap::new();
                values.insert(
                    annotations::ORG_OPENCONTAINERS_IMAGE_TITLE.to_string(),
                    module.clone(),
                );
                Some(values)
            } else {
                let mut values: HashMap<String, String> = HashMap::new();
                for annotation in annotations {
                    let tmp: Vec<_> = annotation.splitn(2, '=').collect();
                    if tmp.len() == 2 {
                        values.insert(String::from(tmp[0]), String::from(tmp[1]));
                    }
                }
                if !values.contains_key(&annotations::ORG_OPENCONTAINERS_IMAGE_TITLE.to_string()) {
                    values.insert(
                        annotations::ORG_OPENCONTAINERS_IMAGE_TITLE.to_string(),
                        module.clone(),
                    );
                }

                Some(values)
            };

            push_wasm(&mut client, &auth, &reference, &module, annotations).await;
        }
    }
}
