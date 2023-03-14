extern crate procfs;

/// List processes using posix shared memory segments

fn main() {
    let shared_memory_vec = procfs::Shm::new().unwrap();

    for shared_memory in &shared_memory_vec {
        println!("key: {}, shmid: {}", shared_memory.key, shared_memory.shmid);
        println!("============");

        for prc in procfs::process::all_processes().unwrap() {
            match prc.smaps() {
                Ok(memory_maps) => {
                    for (memory_map, _memory_map_data) in &memory_maps {
                        if let procfs::process::MMapPath::Vsys(key) = memory_map.pathname {
                            if key == shared_memory.key && memory_map.inode == shared_memory.shmid {
                                println!("{}: {:?}", prc.pid, prc.cmdline().unwrap());
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        println!();
    }
}
