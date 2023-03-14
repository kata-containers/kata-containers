use std::{
    io::Write,
    {env, io},
};

use tokio_uring::fs::File;

fn main() {
    // The file to `cat` is passed as a CLI argument
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no path specified");
    }

    let path = &args[1];

    // Lock stdout
    let out = io::stdout();
    let mut out = out.lock();

    tokio_uring::start(async {
        // Open the file without blocking
        let file = File::open(path).await.unwrap();
        let mut buf = vec![0; 16 * 1_024];

        // Track the current position in the file;
        let mut pos = 0;

        loop {
            // Read a chunk
            let (res, b) = file.read_at(buf, pos).await;
            let n = res.unwrap();

            if n == 0 {
                break;
            }

            out.write_all(&b[..n]).unwrap();
            pos += n as u64;

            buf = b;
        }

        // Include a new line
        println!("");
    });
}
