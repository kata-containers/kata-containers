/*!
A simple example showing how to introspect all available capabilities.

This differentiates between:
 * library set: the static set hardcoded in the library at build-time.
 * runtime set: the dynamic set available in the running kernel.

The runtime set can be introspected in two different ways: via procfs
(faster but not always possible) or via probing (slower). Where both
mechanisms are available, they produce the exact same result.
!*/

type ExResult<T> = Result<T, Box<dyn std::error::Error + 'static>>;

fn main() -> ExResult<()> {
    let library_set = caps::all();
    let runtime_set = caps::runtime::procfs_all_supported(None).unwrap_or_else(|e| {
        eprintln!("procfs introspection failed: {}", e);
        caps::runtime::thread_all_supported()
    });

    println!("Capabilities tally:");
    println!(" -> known by this library: {}.", library_set.len());
    println!(" -> known by the current kernel: {}.\n", runtime_set.len());

    println!("Library vs kernel differences:");
    println!(
        " -> newer caps only in the library set: {:?}.",
        library_set.difference(&runtime_set)
    );
    println!(
        " -> newer caps only in the current kernel set: {:?}.",
        runtime_set.difference(&library_set)
    );

    Ok(())
}
