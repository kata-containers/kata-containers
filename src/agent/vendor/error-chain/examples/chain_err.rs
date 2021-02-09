//! Demonstrates usage of `Error::caused` method. This method enables chaining errors
//! like `ResultExt::chain_err` but doesn't require the presence of a `Result` wrapper.

#[macro_use]
extern crate error_chain;

use std::fs::File;

mod errors {
    use super::LaunchStage;
    use std::io;

    error_chain! {
        foreign_links {
            Io(io::Error) #[doc = "Error during IO"];
        }

        errors {
            Launch(phase: LaunchStage) {
                description("An error occurred during startup")
                display("Startup aborted: {:?} did not complete successfully", phase)
            }

            ConfigLoad(path: String) {
                description("Config file not found")
                display("Unable to read file `{}`", path)
            }
        }
    }

    impl From<LaunchStage> for ErrorKind {
        fn from(v: LaunchStage) -> Self {
            ErrorKind::Launch(v)
        }
    }
}

pub use errors::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchStage {
    ConfigLoad,
    ConfigParse,
    ConfigResolve,
}

/// Read the service config from the file specified.
fn load_config(rel_path: &str) -> Result<()> {
    File::open(rel_path)
        .map(|_| ())
        .chain_err(|| ErrorKind::ConfigLoad(rel_path.to_string()))
}

/// Launch the service.
fn launch(rel_path: &str) -> Result<()> {
    load_config(rel_path).map_err(|e| match e {
        e @ Error(ErrorKind::ConfigLoad(_), _) => e.chain_err(|| LaunchStage::ConfigLoad),
        e => e.chain_err(|| "Unknown failure"),
    })
}

fn main() {
    let chain = launch("does_not_exist.json").unwrap_err();
    for err in chain.iter() {
        println!("{}", err);
    }
}
