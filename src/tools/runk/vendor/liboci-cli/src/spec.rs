use clap::Parser;

/// Command generates a config.json
#[derive(Parser, Debug)]
pub struct Spec {
    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}
