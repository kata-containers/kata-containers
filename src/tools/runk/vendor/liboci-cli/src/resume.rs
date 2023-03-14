use clap::Parser;

/// Resume the processes within the container
#[derive(Parser, Debug)]
pub struct Resume {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}
