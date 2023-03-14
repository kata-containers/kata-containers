use clap::Parser;
use std::path::PathBuf;

/// Create a container and immediately start it
#[derive(Parser, Debug)]
pub struct Run {
    /// File to write pid of the container created
    // note that in the end, container is just another process
    #[clap(short, long)]
    pub pid_file: Option<PathBuf>,
    /// path to the bundle directory, containing config.json and root filesystem
    #[clap(short, long, default_value = ".")]
    pub bundle: PathBuf,
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(short, long)]
    pub console_socket: Option<PathBuf>,
    /// Pass N additional file descriptors to the container (stdio + $LISTEN_FDS + N in total)
    #[clap(long, default_value = "0")]
    pub preserve_fds: i32,
    /// name of the container instance to be started
    #[clap(forbid_empty_values = true, required = true)]
    pub container_id: String,
}
