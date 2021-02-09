use slog_scope;

pub fn simulate_server() {

    let log = slog_scope::logger();

    let server = log.new(o!("host" => "localhost", "port" => "8080"));
    let peer1 = server.new(o!("peer_addr" => "8.8.8.8", "port" => "18230"));
    let peer2 = server.new(o!("peer_addr" => "82.9.9.9", "port" => "42381"));

    info!(server, "starting");
    info!(server, "listening");
    debug!(peer2, "connected");
    debug!(peer2, "message received"; "length" => 2);
    debug!(peer1, "connected");
    warn!(peer2, "weak encryption requested"; "algo" => "xor");
    debug!(peer2, "response sent"; "length" => 8);
    debug!(peer2, "disconnected");
    debug!(peer1, "message received"; "length" => 2);
    debug!(peer1, "response sent"; "length" => 8);
    debug!(peer1, "disconnected");
    crit!(server, "internal error");
    info!(server, "exit");

}
