// SPDX-License-Identifier: MIT

use std::env;

use rtnetlink::new_connection;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        usage();
        return Ok(());
    }

    let index: u32 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("invalid index");
        std::process::exit(1);
    });

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    if let Err(e) = handle.qdisc().add(index as i32).ingress().execute().await {
        eprintln!("{}", e);
    }

    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example add_tc_qdisc_ingress -- <index>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example add_tc_qdisc_ingress 

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./add_tc_qdisc_ingress <index>"
    );
}
