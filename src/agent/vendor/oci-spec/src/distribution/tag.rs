//! Tag types of the distribution spec.

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
/// A list of tags for a given repository.
pub struct TagList {
    /// The namespace of the repository.
    name: String,

    /// Each tags on the repository.
    tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    #[test]
    fn tag_list_success() -> Result<()> {
        let list = TagListBuilder::default()
            .name("name")
            .tags(vec![])
            .build()?;
        assert!(list.tags().is_empty());
        assert_eq!(list.name(), "name");
        Ok(())
    }

    #[test]
    fn tag_list_failure() {
        assert!(TagListBuilder::default().build().is_err());
    }
}
