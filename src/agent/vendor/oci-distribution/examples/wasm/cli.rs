use clap::{Parser, Subcommand};

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

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    #[clap(arg_required_else_help = true)]
    Pull {
        /// Write contents to file
        #[clap(short, long)]
        output: String,

        /// Name of the image to pull
        image: String,
    },
    #[clap(arg_required_else_help = true)]
    Push {
        /// OCI Annotations to be added to the manifest
        #[clap(short, long, required(false))]
        annotations: Vec<String>,

        /// Wasm file to push
        module: String,

        /// Name of the image to pull
        image: String,
    },
}
