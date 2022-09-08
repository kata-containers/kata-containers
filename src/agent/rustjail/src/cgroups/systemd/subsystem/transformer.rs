use super::super::common::{CgroupHierarchy, Properties};
use anyhow::Result;
use oci::LinuxResources;

pub trait Transformer {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        cgroup_hierarchy: &CgroupHierarchy,
        systemd_version: &str,
    ) -> Result<()>;
}
