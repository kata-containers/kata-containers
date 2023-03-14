use clap::Parser;

/// Suspend the processes within the container
#[derive(Parser, Debug)]
pub struct Pause {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}
