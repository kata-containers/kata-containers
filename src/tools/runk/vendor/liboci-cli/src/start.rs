use clap::Parser;

/// Start a previously created container
#[derive(Parser, Debug)]
pub struct Start {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}
