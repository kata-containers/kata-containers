use std::fmt::Debug;
use std::path::PathBuf;

use clap::Parser;

// Subcommands that are specified in https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md

mod create;
mod delete;
mod kill;
mod start;
mod state;

pub use {create::Create, delete::Delete, kill::Kill, start::Start, state::State};

// Other common subcommands that aren't specified in the document
mod checkpoint;
mod events;
mod exec;
mod list;
mod pause;
mod ps;
mod resume;
mod run;
mod spec;
mod update;

pub use {
    checkpoint::Checkpoint, events::Events, exec::Exec, list::List, pause::Pause, ps::Ps,
    resume::Resume, run::Run, spec::Spec, update::Update,
};

// Subcommands parsed by liboci-cli, based on the [OCI
// runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// and specifically the [OCI Command Line
// Interface](https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md)
#[derive(Parser, Debug)]
pub enum StandardCmd {
    Create(Create),
    Start(Start),
    State(State),
    Kill(Kill),
    Delete(Delete),
}

// Extra subcommands not documented in the OCI Command Line Interface,
// but found in
// [runc](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
// and other runtimes.
#[derive(Parser, Debug)]
pub enum CommonCmd {
    Checkpointt(Checkpoint),
    Events(Events),
    Exec(Exec),
    List(List),
    Pause(Pause),
    #[clap(allow_hyphen_values = true)]
    Ps(Ps),
    Resume(Resume),
    Run(Run),
    Update(Update),
    Spec(Spec),
}

// The OCI Command Line Interface document doesn't define any global
// flags, but these are commonly accepted by runtimes
#[derive(Parser, Debug)]
pub struct GlobalOpts {
    /// change log level to debug.
    // Example in future : '--debug     change log level to debug. (default: "warn")'
    #[clap(long)]
    pub debug: bool,
    #[clap(short, long)]
    pub log: Option<PathBuf>,
    #[clap(long)]
    pub log_format: Option<String>,
    /// root directory to store container state
    #[clap(short, long)]
    pub root: Option<PathBuf>,
    /// Enable systemd cgroup manager, rather then use the cgroupfs directly.
    #[clap(short, long)]
    pub systemd_cgroup: bool,
}
