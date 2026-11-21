// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Synchronous OVSDB JSON-RPC transport over a Unix socket.

use std::io::{self, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use socket2::{Domain, SockAddr, Socket, Type};

use crate::OvsError;

const OVSDB_DATABASE: &str = "Open_vSwitch";
const OVSDB_SOCKET: &str = "/run/openvswitch/db.sock";
const OVSDB_REQUEST_ID: i64 = 1;
const OVSDB_IO_TIMEOUT: Duration = Duration::from_secs(35);
// Includes echo requests received before the transaction result.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

pub trait OvsDbClient: Send + Sync {
    /// Return one checked result per operation, or an error.
    fn transact(&self, operations: Vec<Value>) -> Result<Vec<Value>, OvsError>;
}

#[derive(Clone, Debug)]
pub struct UnixOvsDbClient {
    socket_path: PathBuf,
    io_timeout: Duration,
}

impl Default for UnixOvsDbClient {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from(OVSDB_SOCKET),
            io_timeout: OVSDB_IO_TIMEOUT,
        }
    }
}

impl UnixOvsDbClient {
    /// Timeout for the complete RPC, including connection and echo handling.
    pub fn new(socket_path: PathBuf, io_timeout: Duration) -> Self {
        Self {
            socket_path,
            io_timeout,
        }
    }

    fn connect(&self, deadline: Instant) -> Result<UnixStream, OvsError> {
        let address = SockAddr::unix(&self.socket_path).map_err(|error| {
            OvsError::InvalidInput(format!(
                "invalid OVSDB socket path `{}`: {error}",
                self.socket_path.display()
            ))
        })?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None).map_err(|error| {
            operation_failed(format!("failed to create OVSDB client socket: {error}"))
        })?;
        socket
            .connect_timeout(&address, remaining_time(deadline).map_err(io_error)?)
            .map_err(|error| {
                operation_failed(format!(
                    "failed to connect to OVSDB socket `{}`: {error}",
                    self.socket_path.display()
                ))
            })?;
        Ok(UnixStream::from(OwnedFd::from(socket)))
    }

    fn request(&self, parameters: Vec<Value>) -> Result<Value, OvsError> {
        let deadline = Instant::now()
            .checked_add(self.io_timeout)
            .ok_or_else(|| OvsError::InvalidInput("OVSDB timeout is too large".to_string()))?;
        remaining_time(deadline).map_err(io_error)?;
        let request = json!({
            "method": "transact",
            "params": parameters,
            "id": OVSDB_REQUEST_ID,
        });

        let stream = self.connect(deadline)?;
        let mut writer = DeadlineIo::new(&stream, deadline);
        serde_json::to_writer(&mut writer, &request).map_err(|error| {
            operation_failed(format!("failed to send OVSDB transaction: {error}"))
        })?;

        // OVSDB messages have no newline or length prefix. A streaming JSON
        // decoder handles both fragmented and immediately adjacent objects.
        let reader = BufReader::new(DeadlineIo::new(&stream, deadline));
        let mut messages = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();
        loop {
            remaining_time(deadline).map_err(io_error)?;
            let message = messages
                .next()
                .ok_or_else(|| operation_failed("OVSDB closed the connection without a response"))?
                .map_err(|error| {
                    operation_failed(format!("failed to decode OVSDB JSON-RPC response: {error}"))
                })?;
            remaining_time(deadline).map_err(io_error)?;

            if let Some(method) = message.get("method") {
                if method.as_str() == Some("echo") {
                    respond_to_echo(&mut writer, &message)?;
                    continue;
                }
                return Err(operation_failed(format!(
                    "OVSDB sent an unexpected JSON-RPC method: {method}"
                )));
            }

            return decode_rpc_response(message);
        }
    }
}

impl OvsDbClient for UnixOvsDbClient {
    fn transact(&self, operations: Vec<Value>) -> Result<Vec<Value>, OvsError> {
        let operation_count = operations.len();
        let mut parameters = Vec::with_capacity(operation_count + 1);
        parameters.push(Value::String(OVSDB_DATABASE.to_string()));
        parameters.extend(operations);
        let result = self.request(parameters)?;
        decode_transaction_result(result, operation_count)
    }
}

/// Applies the remaining RPC deadline to each read and write.
struct DeadlineIo<'a> {
    stream: &'a UnixStream,
    deadline: Instant,
    received: usize,
}

impl<'a> DeadlineIo<'a> {
    fn new(stream: &'a UnixStream, deadline: Instant) -> Self {
        Self {
            stream,
            deadline,
            received: 0,
        }
    }
}

impl Read for DeadlineIo<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let capacity = MAX_RESPONSE_BYTES.saturating_sub(self.received);
        if capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "OVSDB response exceeds the 16 MiB limit",
            ));
        }
        let length = buffer.len().min(capacity);
        loop {
            self.stream
                .set_read_timeout(Some(remaining_time(self.deadline)?))?;
            match self.stream.read(&mut buffer[..length]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => {
                    self.received += result.as_ref().copied().unwrap_or(0);
                    return result.map_err(normalize_timeout);
                }
            }
        }
    }
}

impl Write for DeadlineIo<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        loop {
            self.stream
                .set_write_timeout(Some(remaining_time(self.deadline)?))?;
            match self.stream.write(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result.map_err(normalize_timeout),
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        remaining_time(self.deadline).map(|_| ())
    }
}

fn remaining_time(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "OVSDB transaction deadline expired",
            )
        })
}

fn normalize_timeout(error: io::Error) -> io::Error {
    if matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "OVSDB transaction deadline expired",
        )
    } else {
        error
    }
}

fn io_error(error: io::Error) -> OvsError {
    operation_failed(error.to_string())
}

fn respond_to_echo(writer: &mut impl Write, request: &Value) -> Result<(), OvsError> {
    let id = request
        .get("id")
        .filter(|id| id.is_string() || id.is_number())
        .ok_or_else(|| operation_failed("OVSDB echo request has no valid id"))?;
    let parameters = request
        .get("params")
        .filter(|parameters| parameters.is_array())
        .ok_or_else(|| operation_failed("OVSDB echo request has no params array"))?;
    let response = json!({"result": parameters, "error": Value::Null, "id": id});
    serde_json::to_writer(writer, &response)
        .map_err(|error| operation_failed(format!("failed to send OVSDB echo response: {error}")))
}

fn decode_rpc_response(response: Value) -> Result<Value, OvsError> {
    let object = response
        .as_object()
        .ok_or_else(|| operation_failed("OVSDB returned a non-object JSON-RPC response"))?;
    if object.get("id") != Some(&json!(OVSDB_REQUEST_ID)) {
        return Err(operation_failed(format!(
            "OVSDB returned a response with unexpected id {:?}",
            object.get("id")
        )));
    }
    if let Some(error) = object.get("error").filter(|error| !error.is_null()) {
        return Err(operation_failed(format!(
            "OVSDB JSON-RPC transaction failed: {}",
            error_text(error)
        )));
    }

    object
        .get("result")
        .cloned()
        .ok_or_else(|| operation_failed("OVSDB response has no result"))
}

pub(crate) fn decode_transaction_result(
    result: Value,
    operation_count: usize,
) -> Result<Vec<Value>, OvsError> {
    let Value::Array(results) = result else {
        return Err(operation_failed(
            "OVSDB transaction response has no result array",
        ));
    };
    // Report operation errors before checking trailing null results.
    for (index, result) in results.iter().enumerate() {
        if let Some(error) = result.get("error").filter(|error| !error.is_null()) {
            let details = result
                .get("details")
                .filter(|details| !details.is_null())
                .map(|details| format!(": {}", error_text(details)))
                .unwrap_or_default();
            let message = format!(
                "OVSDB operation {index} failed with {}{details}",
                error_text(error)
            );
            // A zero-timeout wait failure is safe to retry; I/O timeouts may have committed.
            return Err(if error.as_str() == Some("timed out") {
                OvsError::TransactionConflict(message)
            } else {
                operation_failed(message)
            });
        }
    }
    if results.len() != operation_count {
        return Err(operation_failed(format!(
            "OVSDB returned {} operation results, expected {operation_count}",
            results.len()
        )));
    }
    if results.iter().any(|result| !result.is_object()) {
        return Err(operation_failed(
            "OVSDB transaction contains a malformed operation result",
        ));
    }
    Ok(results)
}

fn error_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn operation_failed(message: impl Into<String>) -> OvsError {
    OvsError::OperationFailed(message.into())
}

#[cfg(test)]
mod tests {
    //! Real Unix-stream framing and error classification; no OVS installation needed.

    use std::fs;
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use crate::{OvsDbClient, OvsError, UnixOvsDbClient};
    use serde_json::{json, Value};

    static NEXT_SOCKET: AtomicUsize = AtomicUsize::new(0);

    struct Server {
        path: PathBuf,
        worker: Option<JoinHandle<()>>,
    }

    impl Server {
        fn start(handler: impl FnOnce(UnixStream) + Send + 'static) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dan-ovs-rpc-{}-{}.sock",
                std::process::id(),
                NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
            ));
            let listener = UnixListener::bind(&path).expect("isolated listener");
            let worker = thread::spawn(move || {
                let (stream, _) = listener.accept().expect("client connection");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("read deadline");
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .expect("write deadline");
                handler(stream);
            });
            Self {
                path,
                worker: Some(worker),
            }
        }

        fn client(&self) -> UnixOvsDbClient {
            UnixOvsDbClient::new(self.path.clone(), Duration::from_secs(2))
        }

        fn finish(mut self) {
            self.worker
                .take()
                .expect("worker")
                .join()
                .expect("server assertions");
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn fragmented_rpc_stream() {
        let operation = json!({"op": "select", "table": "Bridge", "where": []});
        let expected_operation = operation.clone();
        let server = Server::start(move |mut stream| {
            let reader = stream.try_clone().expect("reader");
            let mut messages = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();
            let request = messages.next().expect("request").expect("JSON request");
            assert_eq!(request["method"], "transact");
            assert_eq!(
                request["params"],
                json!(["Open_vSwitch", expected_operation])
            );
            let echo = serde_json::to_vec(
                &json!({"method": "echo", "params": ["keepalive"], "id": "echo-1"}),
            )
            .expect("echo JSON");
            for fragment in echo.chunks(3) {
                stream.write_all(fragment).expect("fragment");
            }
            // No newline or length marker separates the echo from the RPC reply.
            serde_json::to_writer(
                &mut stream,
                &json!({"result": [{"rows": []}], "error": null, "id": request["id"]}),
            )
            .expect("response");
            let response = messages.next().expect("echo response").expect("echo JSON");
            assert_eq!(
                response,
                json!({"result": ["keepalive"], "error": null, "id": "echo-1"})
            );
        });
        assert_eq!(
            server.client().transact(vec![operation]),
            Ok(vec![json!({"rows": []})])
        );
        server.finish();
    }

    fn transaction_reply(result: Value, count: usize) -> Result<Vec<Value>, OvsError> {
        let server = Server::start(move |mut stream| {
            let request =
                serde_json::Deserializer::from_reader(stream.try_clone().expect("reader"))
                    .into_iter::<Value>()
                    .next()
                    .expect("request")
                    .expect("JSON");
            serde_json::to_writer(
                &mut stream,
                &json!({"result": result, "error": null, "id": request["id"]}),
            )
            .expect("response");
        });
        let result = server
            .client()
            .transact(vec![json!({"op": "comment", "comment": "contract"}); count]);
        server.finish();
        result
    }

    #[test]
    fn classify_transaction_errors() {
        let conflict = transaction_reply(
            json!([{}, {"error": "timed out", "details": "wait snapshot changed"}, null]),
            3,
        );
        assert!(matches!(conflict, Err(OvsError::TransactionConflict(_))));
        let failure = transaction_reply(
            json!([{"error": "constraint violation", "details": "duplicate name"}, null]),
            2,
        );
        assert!(matches!(failure, Err(OvsError::OperationFailed(_))));
    }
}
