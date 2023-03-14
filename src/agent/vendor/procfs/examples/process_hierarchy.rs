use procfs::process::{all_processes, Stat};

struct ProcessEntry {
    stat: Stat,
    cmdline: Option<Vec<String>>,
}

/// Print all processes as a tree.
/// The tree reflects the hierarchical relationship between parent and child processes.
fn main() {
    // Get all processes
    let processes: Vec<ProcessEntry> = match all_processes() {
        Err(err) => {
            println!("Failed to read all processes: {}", err);
            return;
        }
        Ok(processes) => processes,
    }
    .filter_map(|v| {
        v.and_then(|p| {
            let stat = p.stat()?;
            let cmdline = p.cmdline().ok();
            Ok(ProcessEntry { stat, cmdline })
        })
        .ok()
    })
    .collect();
    // Iterate through all processes and start with top-level processes.
    // Those can be identified by checking if their parent PID is zero.
    for process in &processes {
        if process.stat.ppid == 0 {
            print_process(process, &processes, 0);
        }
    }
}

/// Take a process, print its command and recursively list all child processes.
/// This function will call itself until no further children can be found.
/// It's a depth-first tree exploration.
///
/// depth: The hierarchical depth of the process
fn print_process(process: &ProcessEntry, all_processes: &Vec<ProcessEntry>, depth: usize) {
    let cmdline = match &process.cmdline {
        Some(cmdline) => cmdline.join(" "),
        None => "zombie process".into(),
    };

    // Some processes seem to have an empty cmdline.
    if cmdline.is_empty() {
        return;
    }

    // 10 characters width for the pid
    let pid_length = 8;
    let mut pid = process.stat.pid.to_string();
    pid.push_str(&" ".repeat(pid_length - pid.len()));

    let padding = " ".repeat(4 * depth);
    println!("{}{}{}", pid, padding, cmdline);

    let children = get_children(process.stat.pid, all_processes);
    for child in &children {
        print_process(child, all_processes, depth + 1);
    }
}

/// Get all children of a specific process, by iterating through all processes and
/// checking their parent pid.
fn get_children(pid: i32, all_processes: &[ProcessEntry]) -> Vec<&ProcessEntry> {
    all_processes
        .iter()
        .filter(|process| process.stat.ppid == pid)
        .collect()
}
