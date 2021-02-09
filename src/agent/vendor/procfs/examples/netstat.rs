#![allow(clippy::print_literal)]

extern crate procfs;

use procfs::process::{FDTarget, Process};

use std::collections::HashMap;

fn main() {
    // get all processes
    let all_procs = procfs::process::all_processes().unwrap();

    // build up a map between socket inodes and processes:
    let mut map: HashMap<u32, &Process> = HashMap::new();
    for process in &all_procs {
        if let Ok(fds) = process.fd() {
            for fd in fds {
                if let FDTarget::Socket(inode) = fd.target {
                    map.insert(inode, process);
                }
            }
        }
    }

    // get the tcp table
    let tcp = procfs::net::tcp().unwrap();
    let tcp6 = procfs::net::tcp6().unwrap();

    println!(
        "{:<26} {:<26} {:<15} {:<8} {}",
        "Local address", "Remote address", "State", "Inode", "PID/Program name"
    );

    for entry in tcp.into_iter().chain(tcp6) {
        // find the process (if any) that has an open FD to this entry's inode
        let local_address = format!("{}", entry.local_address);
        let remote_addr = format!("{}", entry.remote_address);
        let state = format!("{:?}", entry.state);
        if let Some(process) = map.get(&entry.inode) {
            println!(
                "{:<26} {:<26} {:<15} {:<12} {}/{}",
                local_address, remote_addr, state, entry.inode, process.stat.pid, process.stat.comm
            );
        } else {
            // We might not always be able to find the process assocated with this socket
            println!(
                "{:<26} {:<26} {:<15} {:<12} -",
                local_address, remote_addr, state, entry.inode
            );
        }
    }
}
