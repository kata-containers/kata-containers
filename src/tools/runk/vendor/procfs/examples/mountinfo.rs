use procfs::process::Process;
use std::collections::HashSet;

fn main() {
    for mount in Process::myself().unwrap().mountinfo().unwrap() {
        let (a, b): (HashSet<_>, HashSet<_>) = mount
            .mount_options
            .into_iter()
            .chain(mount.super_options)
            .partition(|&(_, ref m)| m.is_none());

        println!(
            "{} on {} type {} ({})",
            mount.mount_source.unwrap_or_else(|| "None".to_string()),
            mount.mount_point.display(),
            mount.fs_type,
            a.into_iter().map(|(k, _)| k).collect::<Vec<_>>().join(",")
        );

        for (opt, val) in b {
            if let Some(val) = val {
                println!("  {} = {}", opt, val);
            } else {
                println!("  {}", opt);
            }
        }
    }
}
