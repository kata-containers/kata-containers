#[cfg(unix)]
mod unix_limits {
    use std::cmp;
    use std::io;

    use rlimit::Resource;

    const DEFAULT_NOFILE_LIMIT: u64 = 16384; // or another number

    /// Try to increase NOFILE limit and return the current soft limit.
    pub fn increase_nofile_limit() -> io::Result<u64> {
        let (soft, hard) = Resource::NOFILE.get()?;
        println!("Before increasing: soft   = {}, hard = {}", soft, hard);

        let target = cmp::min(DEFAULT_NOFILE_LIMIT, hard);
        println!("Try to increase:   target = {}", target);
        Resource::NOFILE.set(target, hard)?;

        let (soft, hard) = Resource::NOFILE.get()?;
        println!("After increasing:  soft   = {}, hard = {}", soft, hard);
        Ok(soft)
    }
}

fn main() {
    #[cfg(unix)]
    {
        match unix_limits::increase_nofile_limit() {
            Ok(soft) => println!("NOFILE limit:      soft   = {}", soft),
            Err(err) => println!("Failed to increase NOFILE limit: {}", err),
        }
    }
    #[cfg(not(unix))]
    {
        println!("Do nothing on non-Unix systems");
    }
}
