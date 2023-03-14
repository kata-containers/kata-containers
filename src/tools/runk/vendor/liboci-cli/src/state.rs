use clap::Parser;

/// Show the container state
#[derive(Parser, Debug)]
pub struct State {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}
