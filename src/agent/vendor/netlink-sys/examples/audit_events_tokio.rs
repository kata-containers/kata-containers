// Build:
//
// ```
// cd netlink-sys
// cargo build --example audit_events_async --features tokio_socket
// ```
//
// Run *as root*:
//
// ```
// ../target/debug/examples/audit_events_async
// ```

use std::process;

use netlink_packet_audit::{
    AuditMessage,
    NetlinkBuffer,
    NetlinkMessage,
    StatusMessage,
    NLM_F_ACK,
    NLM_F_REQUEST,
};

use netlink_sys::{protocols::NETLINK_AUDIT, SocketAddr, TokioSocket};

const AUDIT_STATUS_ENABLED: u32 = 1;
const AUDIT_STATUS_PID: u32 = 4;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kernel_unicast: SocketAddr = SocketAddr::new(0, 0);
    let mut socket = TokioSocket::new(NETLINK_AUDIT).unwrap();

    let mut status = StatusMessage::new();
    status.enabled = 1;
    status.pid = process::id();
    status.mask = AUDIT_STATUS_ENABLED | AUDIT_STATUS_PID;
    let payload = AuditMessage::SetStatus(status);
    let mut nl_msg = NetlinkMessage::from(payload);
    nl_msg.header.flags = NLM_F_REQUEST | NLM_F_ACK;
    nl_msg.finalize();

    let mut buf = vec![0; 1024 * 8];
    nl_msg.serialize(&mut buf[..nl_msg.buffer_len()]);

    println!(">>> {:?}", nl_msg);
    socket
        .send_to(&buf[..nl_msg.buffer_len()], &kernel_unicast)
        .await
        .unwrap();

    let mut buf = vec![0; 1024 * 8];
    loop {
        let (n, _addr) = socket.recv_from(&mut buf).await.unwrap();
        // This dance with the NetlinkBuffer should not be
        // necessary. It is here to work around a netlink bug. See:
        // https://github.com/mozilla/libaudit-go/issues/24
        // https://github.com/linux-audit/audit-userspace/issues/78
        {
            let mut nl_buf = NetlinkBuffer::new(&mut buf[0..n]);
            if n != nl_buf.length() as usize {
                nl_buf.set_length(n as u32);
            }
        }
        let parsed = NetlinkMessage::<AuditMessage>::deserialize(&buf[0..n]).unwrap();
        println!("<<< {:?}", parsed);
    }
}
