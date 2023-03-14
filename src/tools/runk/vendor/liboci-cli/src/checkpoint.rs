use clap::Parser;
use std::path::PathBuf;

/// Checkpoint a running container
#[derive(Parser, Debug)]
pub struct Checkpoint {
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
    /// Allow external unix sockets
    #[clap(long)]
    pub ext_unix_sk: bool,
    /// Allow file locks
    #[clap(long)]
    pub file_locks: bool,
    /// Path for saving criu image files
    #[clap(long, default_value = "checkpoint")]
    pub image_path: PathBuf,
    /// Leave the process running after checkpointing
    #[clap(long)]
    pub leave_running: bool,
    /// Allow shell jobs
    #[clap(long)]
    pub shell_job: bool,
    /// Allow open tcp connections
    #[clap(long)]
    pub tcp_established: bool,
    /// Path for saving work files and logs
    #[clap(long)]
    pub work_path: Option<PathBuf>,
}
