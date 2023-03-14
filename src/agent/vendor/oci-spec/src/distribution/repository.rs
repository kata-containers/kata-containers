//! Repository types of the distribution spec.

use crate::error::OciSpecError;
use derive_builder::Builder;
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Builder, Clone, Debug, Deserialize, Eq, Getters, Setters, PartialEq, Serialize)]
#[builder(
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// RepositoryList returns a catalog of repositories maintained on the registry.
pub struct RepositoryList {
    /// The items of the RepositoryList.
    repositories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    #[test]
    fn repository_list_success() -> Result<()> {
        let list = RepositoryListBuilder::default()
            .repositories(vec![])
            .build()?;
        assert!(list.repositories().is_empty());
        Ok(())
    }

    #[test]
    fn repository_list_failure() {
        assert!(RepositoryListBuilder::default().build().is_err());
    }
}
