//! Example of how to implement `KV` for a struct
//! to conveniently log data associated with it.
#[macro_use]
extern crate slog;
use slog::*;

mod common;

struct Peer {
    host: String,
    port: u32,
}

impl Peer {
    fn new(host: String, port: u32) -> Self {
        Peer {
            host: host,
            port: port,
        }
    }
}

// `KV` can be implemented for a struct
impl KV for Peer {
    fn serialize(&self, _record: &Record, serializer: &mut Serializer) -> Result {
        serializer.emit_u32(Key::from("peer-port"), self.port)?;
        serializer.emit_str(Key::from("peer-host"), &self.host)?;
        Ok(())
    }
}

struct Server {
    _host: String,
    _port: u32,
    // One approach is to create new `Logger` with struct data
    // and embedded it into struct itself.  This works when struct is mostly
    // immutable.
    log: Logger,
}

impl Server {
    fn new(host: String, port: u32, log: Logger) -> Server {
        let log = log.new(o!("server-host" => host.clone(), "server-port" => port));
        Server {
            _host: host,
            _port: port,
            log: log,
        }
    }

    fn connection(&self, peer: &Peer) {
        // Another approach is to add struct to a logging message when it's
        // necessary. This might be necessary when struct data can change
        // between different logging statements (not the case here for `Peer`).
        info!(self.log, "new connection"; peer);
    }
}

struct PeerCounter {
    count: usize,
    log: Logger,
}

impl PeerCounter {
    fn new(log: Logger) -> Self {
        PeerCounter { count: 0, log: log }
    }

    // A hybrid approach with `Logger` with parent logging-context embedded into
    // a `struct` and a helper function adding mutable fields.
    fn log_info(&self, msg: &str, kv: BorrowedKV) {
        info!(self.log, "{}", msg; "current-count" => self.count, kv);
    }

    fn count(&mut self, peer: &Peer) {
        self.count += 1;
        self.log_info("counted peer", b!(peer));
    }
}

fn main() {
    let log = Logger::root(Fuse(common::PrintlnDrain), o!("build-id" => "7.3.3-abcdef"));

    let server = Server::new("localhost".into(), 12345, log.clone());

    let peer = Peer::new("1.2.3.4".into(), 999);
    server.connection(&peer);
    let mut counter = PeerCounter::new(log);
    counter.count(&peer);
}
