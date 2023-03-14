use oci_distribution::{secrets::RegistryAuth, Client, Reference};

use clap::Parser;
use docker_credential::{CredentialRetrievalError, DockerCredential};
use tracing::{debug, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Pull a WebAssembly module from a OCI container registry
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// Enable verbose mode
    #[clap(short, long)]
    pub verbose: bool,

    /// Perform anonymous operation, by default the tool tries to reuse the docker credentials read
    /// from the default docker file
    #[clap(short, long)]
    pub anonymous: bool,

    /// Pull image from registry using HTTP instead of HTTPS
    #[clap(short, long)]
    pub insecure: bool,

    /// Enable json output
    #[clap(long)]
    pub json: bool,

    /// Name of the image to pull
    image: String,
}

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

    let reference: Reference = cli.image.parse().expect("Not a valid image reference");
    let auth = build_auth(&reference, &cli);

    let client_config = build_client_config(&cli);
    let mut client = Client::new(client_config);

    let (manifest, _) = client
        .pull_manifest(&reference, &auth)
        .await
        .expect("Cannot pull manifest");

    if cli.json {
        serde_json::to_writer_pretty(std::io::stdout(), &manifest)
            .expect("Cannot serialize manifest to JSON");
        println!();
    } else {
        println!("{}", manifest);
    }
}
