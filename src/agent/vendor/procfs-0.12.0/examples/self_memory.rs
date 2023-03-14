use procfs::process::Process;

fn main() {
    let me = Process::myself().expect("Unable to load myself!");
    println!("PID: {}", me.pid);

    let page_size = procfs::page_size().expect("Unable to determinte page size!") as u64;
    println!("Memory page size: {}", page_size);

    // Note: when comparing the below values to what "top" will display, note that "top" will use
    // base-2 units (kibibytes), not base-10 units (kilobytes).

    println!("== Data from /proc/self/stat:");
    println!("Total virtual memory used: {} bytes", me.stat.vsize);
    println!(
        "Total resident set: {} pages ({} bytes)",
        me.stat.rss,
        me.stat.rss as u64 * page_size
    );
    println!();

    if let Ok(statm) = me.statm() {
        println!("== Data from /proc/self/statm:");
        println!(
            "Total virtual memory used: {} pages ({} bytes)",
            statm.size,
            statm.size * page_size
        );
        println!(
            "Total resident set: {} pages ({} byte)s",
            statm.resident,
            statm.resident * page_size
        );
        println!(
            "Total shared memory: {} pages ({} bytes)",
            statm.shared,
            statm.shared * page_size
        );
        println!();
    }

    if let Ok(status) = me.status() {
        println!("== Data from /proc/self/status:");
        println!(
            "Total virtual memory used: {} bytes",
            status.vmsize.expect("vmsize") * 1024
        );
        println!("Total resident set: {} bytes", status.vmrss.expect("vmrss") * 1024);
        println!(
            "Total shared memory: {} bytes",
            status.rssfile.expect("rssfile") * 1024 + status.rssshmem.expect("rssshmem") * 1024
        );
    }
}
