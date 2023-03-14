//! # UDP Jaeger Agent Client
use crate::exporter::thrift::{
    agent::{self, TAgentSyncClient},
    jaeger,
};
use crate::exporter::transport::{TBufferChannel, TNoopChannel};
use std::fmt;
use std::net::{ToSocketAddrs, UdpSocket};
use thrift::{
    protocol::{TCompactInputProtocol, TCompactOutputProtocol},
    transport::{ReadHalf, TIoChannel, WriteHalf},
};

/// The max size of UDP packet we want to send, synced with jaeger-agent
const UDP_PACKET_MAX_LENGTH: usize = 65_000;

struct BufferClient {
    buffer: ReadHalf<TBufferChannel>,
    client: agent::AgentSyncClient<
        TCompactInputProtocol<TNoopChannel>,
        TCompactOutputProtocol<WriteHalf<TBufferChannel>>,
    >,
}

impl fmt::Debug for BufferClient {
    /// Debug info
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BufferClient")
            .field("buffer", &self.buffer)
            .field("client", &"AgentSyncClient")
            .finish()
    }
}

/// `AgentAsyncClientUDP` implements an async version of the `TAgentSyncClient`
/// interface over UDP.
#[derive(Debug)]
pub(crate) struct AgentAsyncClientUdp {
    #[cfg(all(not(feature = "async-std"), not(feature = "tokio")))]
    conn: UdpSocket,
    #[cfg(feature = "tokio")]
    conn: tokio::net::UdpSocket,
    #[cfg(all(feature = "async-std", not(feature = "tokio")))]
    conn: async_std::net::UdpSocket,
    buffer_client: BufferClient,
    max_packet_size: usize,
}

impl AgentAsyncClientUdp {
    /// Create a new UDP agent client
    pub(crate) fn new<T: ToSocketAddrs>(
        host_port: T,
        max_packet_size: Option<usize>,
    ) -> thrift::Result<Self> {
        let max_packet_size = max_packet_size.unwrap_or(UDP_PACKET_MAX_LENGTH);
        let (buffer, write) = TBufferChannel::with_capacity(max_packet_size).split()?;
        let client = agent::AgentSyncClient::new(
            TCompactInputProtocol::new(TNoopChannel),
            TCompactOutputProtocol::new(write),
        );

        let conn = UdpSocket::bind("0.0.0.0:0")?;
        conn.connect(host_port)?;

        Ok(AgentAsyncClientUdp {
            #[cfg(all(not(feature = "async-std"), not(feature = "tokio")))]
            conn,
            #[cfg(feature = "tokio")]
            conn: tokio::net::UdpSocket::from_std(conn)?,
            #[cfg(all(feature = "async-std", not(feature = "tokio")))]
            conn: async_std::net::UdpSocket::from(conn),
            buffer_client: BufferClient { buffer, client },
            max_packet_size,
        })
    }

    /// Emit standard Jaeger batch
    pub(crate) async fn emit_batch(&mut self, batch: jaeger::Batch) -> thrift::Result<()> {
        // Write payload to buffer
        self.buffer_client.client.emit_batch(batch)?;
        let payload = self.buffer_client.buffer.take_bytes();

        if payload.len() > self.max_packet_size {
            return Err(thrift::ProtocolError::new(
                thrift::ProtocolErrorKind::SizeLimit,
                format!(
                    "jaeger exporter payload size of {} bytes over max UDP packet size of {} bytes. Try setting a smaller batch size.",
                    payload.len(),
                    self.max_packet_size,
                ),
            )
            .into());
        }

        // Write async to socket, reading from buffer
        write_to_socket(self, payload).await?;

        Ok(())
    }
}

#[cfg(all(not(feature = "async-std"), not(feature = "tokio")))]
async fn write_to_socket(client: &mut AgentAsyncClientUdp, payload: Vec<u8>) -> thrift::Result<()> {
    client.conn.send(&payload)?;

    Ok(())
}

#[cfg(feature = "tokio")]
async fn write_to_socket(client: &mut AgentAsyncClientUdp, payload: Vec<u8>) -> thrift::Result<()> {
    client.conn.send(&payload).await?;

    Ok(())
}

#[cfg(all(feature = "async-std", not(feature = "tokio")))]
async fn write_to_socket(client: &mut AgentAsyncClientUdp, payload: Vec<u8>) -> thrift::Result<()> {
    client.conn.send(&payload).await?;

    Ok(())
}
