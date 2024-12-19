// Copyright 2022 Alibaba Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! # Upcall Client's Implementation
//!
//! Provides basic operations for upcall client, include:
//! - Connect to upcall server and service
//! - Send data to server
//! - Receive data from server

mod dev_mgr_service;

use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dbs_utils::epoll_manager::{EpollManager, EventOps, EventSet, Events, MutEventSubscriber};
use dbs_virtio_devices::vsock::backend::{VsockInnerConnector, VsockStream};
use log::{debug, error, info, trace, warn};
use timerfd::{SetTimeFlags, TimerFd, TimerState};

pub use crate::dev_mgr_service::{
    CpuDevRequest, DevMgrRequest, DevMgrResponse, DevMgrService, MmioDevRequest, PciDevRequest,
};

const SERVER_PORT: u32 = 0xDB;
const SERVER_RECONNECT_DURATION_MS: u64 = 10;
const SERVER_MAX_RECONNECT_TIME: u32 = 500;

/// Upcall client error.
#[derive(Debug, thiserror::Error)]
pub enum UpcallClientError {
    /// Received invalid upcall message.
    #[error("received invalid upcall message: {0}")]
    InvalidMessage(String),
    /// Upcall server connect error.
    #[error("upcall server connect error: {0}")]
    ServerConnect(#[source] std::io::Error),
    /// Upcall service connect error.
    #[error("upcall service connect error: {0}")]
    ServiceConnect(#[source] std::io::Error),
    /// Upcall send request error.
    #[error("upcall send request error: {0}")]
    SendRequest(#[source] std::io::Error),
    /// Upcall get response error.
    #[error("upcall get response error: {0}")]
    GetResponse(#[source] std::io::Error),
    /// Errors with timerfd.
    #[error("timerfd error: {0}")]
    TimerFd(#[source] std::io::Error),
    /// Upcall is not connected.
    #[error("upcall is not connected")]
    UpcallIsNotConnected,
    /// Upcall is busy now.
    #[error("upcall is busy now")]
    UpcallIsBusy,
}

/// Upcall client result.
pub type Result<T> = std::result::Result<T, UpcallClientError>;

/// Upcall client state, used by upcall client state machine.
///
// NOTE: here's not a state like `ServerDisconnect`, because we always connect
// to server immediately when constructing the connection or disconnected from
// server.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum UpcallClientState {
    /// There are two possible scenarios for a connection in this state:
    /// - Server's connection is broken, waiting for reconnect.
    /// - Server connection request sent, waiting for server's response.
    WaitingServer,
    /// Service connection request sent, waiting for service's response.
    WaitingService,
    /// The upcall service is connected.
    ServiceConnected,
    /// The upcall channl is busy (request has been sent, but response has not
    /// been received).
    ServiceBusy,
    /// Error state that cannot just reconnect to server.
    ReconnectError,
}

#[allow(clippy::large_enum_variant)]
/// Upcall client request of different services.
pub enum UpcallClientRequest {
    /// Device manager's request.
    DevMgr(DevMgrRequest),
    #[cfg(test)]
    /// Fake service's request.
    FakeRequest,
}

/// Upcall client response of different services.
#[derive(Debug, Eq, PartialEq)]
pub enum UpcallClientResponse {
    /// Device manager's response.
    DevMgr(DevMgrResponse),
    /// Upcall client disconnected, and need to reconnect.
    UpcallReset,
    #[cfg(test)]
    /// Fake service's response
    FakeResponse,
}

/// Shared info between upcall client and upcall epoll handler.
struct UpcallClientInfo<S: UpcallClientService + Send> {
    service: S,
    connector: VsockInnerConnector,
    stream: Option<Box<dyn VsockStream>>,
    state: UpcallClientState,
    result_callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
}

impl<S: UpcallClientService + Send> UpcallClientInfo<S> {
    fn server_connection_start(&mut self) -> Result<()> {
        let mut stream = self
            .connector
            .connect()
            .map_err(UpcallClientError::ServerConnect)?;
        stream
            .set_nonblocking(true)
            .map_err(UpcallClientError::ServerConnect)?;

        let cmd = format!("CONNECT {SERVER_PORT}\n");
        stream
            .write_all(&cmd.into_bytes())
            .map_err(UpcallClientError::ServerConnect)?;

        // drop the old stream
        let _ = self.stream.replace(stream);

        Ok(())
    }

    fn server_connection_check(&mut self) -> Result<()> {
        let mut buffer = [0; 50];
        let len = self
            .stream
            .as_mut()
            .unwrap()
            .read(&mut buffer)
            .map_err(UpcallClientError::ServerConnect)?;

        if !(len > 2 && buffer[0..2] == [b'O', b'K']) {
            return Err(UpcallClientError::InvalidMessage(format!(
                "upcall server expect ok, but received {}",
                String::from_utf8_lossy(&buffer[0..2]),
            )));
        }

        Ok(())
    }

    fn service_connection_start(&mut self) -> Result<()> {
        self.service.connection_start(self.stream.as_mut().unwrap())
    }

    fn service_connection_check(&mut self) -> Result<()> {
        self.service.connection_check(self.stream.as_mut().unwrap())
    }

    fn send_request(&mut self, request: UpcallClientRequest) -> Result<()> {
        self.service
            .send_request(self.stream.as_mut().unwrap(), request)
    }

    fn handle_response(&mut self) -> Result<UpcallClientResponse> {
        self.service.handle_response(self.stream.as_mut().unwrap())
    }

    fn set_state(&mut self, state: UpcallClientState) {
        self.state = state;
    }

    fn set_callback(&mut self, callback: Box<dyn Fn(UpcallClientResponse) + Send>) {
        self.result_callback.replace(callback);
    }

    fn consume_callback(&mut self, response: UpcallClientResponse) {
        if let Some(cb) = self.result_callback.take() {
            cb(response)
        };
    }
}

/// Upcall client's Implementation.
pub struct UpcallClient<S: UpcallClientService + Send> {
    epoll_manager: EpollManager,
    info: Arc<Mutex<UpcallClientInfo<S>>>,
}

impl<S: UpcallClientService + Send + 'static> UpcallClient<S> {
    /// Create a new Upcall Client instance.
    pub fn new(
        connector: VsockInnerConnector,
        epoll_manager: EpollManager,
        service: S,
    ) -> Result<Self> {
        let info = UpcallClientInfo {
            connector,
            stream: None,
            state: UpcallClientState::WaitingServer,
            service,
            result_callback: None,
        };
        Ok(UpcallClient {
            epoll_manager,
            info: Arc::new(Mutex::new(info)),
        })
    }

    /// Connect upcall client to upcall server.
    pub fn connect(&mut self) -> Result<()> {
        let handler = Box::new(UpcallEpollHandler::new(self.info.clone())?);
        self.epoll_manager.add_subscriber(handler);

        Ok(())
    }

    fn send_request_inner(
        &self,
        request: UpcallClientRequest,
        callback: Option<Box<dyn Fn(UpcallClientResponse) + Send>>,
    ) -> Result<()> {
        let mut info = self.info.lock().unwrap();
        match info.state {
            UpcallClientState::WaitingServer
            | UpcallClientState::WaitingService
            | UpcallClientState::ReconnectError => Err(UpcallClientError::UpcallIsNotConnected),
            UpcallClientState::ServiceBusy => Err(UpcallClientError::UpcallIsBusy),
            UpcallClientState::ServiceConnected => {
                info.send_request(request)?;
                info.set_state(UpcallClientState::ServiceBusy);
                if let Some(cb) = callback {
                    info.set_callback(cb)
                };
                Ok(())
            }
        }
    }

    /// Send request to upcall server, and get the response from callback
    /// function.
    pub fn send_request(
        &self,
        request: UpcallClientRequest,
        callback: Box<dyn Fn(UpcallClientResponse) + Send>,
    ) -> Result<()> {
        self.send_request_inner(request, Some(callback))
    }

    /// Only send request to upcall server, and discard the response.
    pub fn send_request_without_result(&self, request: UpcallClientRequest) -> Result<()> {
        self.send_request_inner(request, None)
    }

    /// Get the link state of upcall client.
    pub fn get_state(&self) -> UpcallClientState {
        self.info.lock().unwrap().state.clone()
    }

    /// The upcall client is ready to send request to upcall server or not.
    pub fn is_ready(&self) -> bool {
        self.get_state() == UpcallClientState::ServiceConnected
    }
}

/// Event handler of upcall client.
pub struct UpcallEpollHandler<S: UpcallClientService + Send> {
    info: Arc<Mutex<UpcallClientInfo<S>>>,
    reconnect_timer: TimerFd,
    reconnect_time: u32,
    in_reconnect: bool,
}

impl<S: UpcallClientService + Send> UpcallEpollHandler<S> {
    fn new(info: Arc<Mutex<UpcallClientInfo<S>>>) -> Result<Self> {
        let handler = UpcallEpollHandler {
            info,
            reconnect_timer: TimerFd::new().map_err(UpcallClientError::TimerFd)?,
            reconnect_time: 0,
            in_reconnect: false,
        };
        let info = handler.info.clone();
        info.lock().unwrap().server_connection_start()?;

        Ok(handler)
    }

    fn set_reconnect(&mut self) -> Result<()> {
        if self.in_reconnect {
            info!("upcall server is waiting for reconnect");
            return Ok(());
        }
        self.in_reconnect = true;

        self.reconnect_timer
            .set_state(TimerState::Disarmed, SetTimeFlags::Default);

        if self.reconnect_time > SERVER_MAX_RECONNECT_TIME {
            error!("upcall server's max reconnect time exceed");
            return Ok(());
        }

        self.reconnect_timer.set_state(
            TimerState::Oneshot(Duration::from_millis(SERVER_RECONNECT_DURATION_MS)),
            SetTimeFlags::Default,
        );

        self.reconnect_time += 1;
        Ok(())
    }

    fn handle_stream_event(&mut self, ops: &mut EventOps) {
        let info = self.info.clone();
        let mut info = info.lock().unwrap();
        match info.state {
            UpcallClientState::WaitingServer => {
                if let Err(e) = info.server_connection_check() {
                    debug!("upcall connect server check failed, {}", e);
                    info.set_state(UpcallClientState::WaitingServer);
                    if let Err(e) = self.set_reconnect() {
                        error!("set reconnect error: {}", e);
                        info.set_state(UpcallClientState::ReconnectError);
                    }
                } else {
                    info!("upcall connect server success");
                    // It's time to connect to service when server is connected.
                    if let Err(e) = info.service_connection_start() {
                        warn!("upcall connect service start failed {}", e);
                        info.set_state(UpcallClientState::WaitingServer);
                        if let Err(e) = self.set_reconnect() {
                            error!("set reconnect error: {}", e);
                            info.set_state(UpcallClientState::ReconnectError);
                        }
                    } else {
                        // only if both server connection check and service connection start are ok, change to next state
                        info.state = UpcallClientState::WaitingService;
                    }
                }
            }
            UpcallClientState::WaitingService => {
                if let Err(e) = info.service_connection_check() {
                    warn!("upcall connect service check failed, {}", e);
                    info.set_state(UpcallClientState::WaitingServer);
                    if let Err(e) = self.set_reconnect() {
                        error!("set reconnect error: {}", e);
                        info.set_state(UpcallClientState::ReconnectError);
                    }
                } else {
                    info!("upcall connect service success");
                    info.set_state(UpcallClientState::ServiceConnected);
                }
            }
            UpcallClientState::ServiceBusy => match info.handle_response() {
                Ok(response) => {
                    trace!("upcall handle response success");
                    info.set_state(UpcallClientState::ServiceConnected);
                    info.consume_callback(response);
                }
                Err(e) => {
                    warn!("upcall response failed {}", e);
                    info.set_state(UpcallClientState::WaitingServer);
                    if let Err(e) = self.set_reconnect() {
                        error!("set reconnect error: {}", e);
                        info.set_state(UpcallClientState::ReconnectError);
                    }
                }
            },
            UpcallClientState::ServiceConnected | UpcallClientState::ReconnectError => {
                error!("we should get message from event handler when connection state is `ServiceConnected`");
            }
        }

        if self.in_reconnect {
            // remove the old stream's fd in epoll and drop the old stream
            if let Some(stream) = info.stream.as_ref() {
                ops.remove(Events::new_raw(stream.as_raw_fd(), EventSet::IN))
                    .unwrap();
            }
            let _ = info.stream.take();

            // consume the result callback before reconnect
            info.consume_callback(UpcallClientResponse::UpcallReset);
        }
    }

    fn handle_reconnect_event(&mut self, ops: &mut EventOps) {
        // we should clear the reconnect timer and flag first
        self.in_reconnect = false;
        self.reconnect_timer
            .set_state(TimerState::Disarmed, SetTimeFlags::Default);

        let info = self.info.clone();
        let mut info = info.lock().unwrap();
        // reconnect to server
        if let Err(e) = info.server_connection_start() {
            warn!("upcall reconnect server /failed: {}", e);
            if let Err(e) = self.set_reconnect() {
                error!("set reconnect error: {}", e);
            }
        }
        debug!("upcall reconnect server...");
        // add new stream's fn to epoll
        if let Some(stream) = info.stream.as_ref() {
            ops.add(Events::new_raw(stream.as_raw_fd(), EventSet::IN))
                .unwrap();
        }
    }
}

impl<S> MutEventSubscriber for UpcallEpollHandler<S>
where
    S: UpcallClientService + Send + 'static,
{
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        trace!("UpcallEpollHandler: process");

        let info = self.info.lock().unwrap();
        let stream_fd = info.stream.as_ref().map(|s| s.as_raw_fd());
        drop(info);

        let reconnect_fd = self.reconnect_timer.as_raw_fd();
        match events.fd() {
            fd if Some(fd) == stream_fd => self.handle_stream_event(ops),
            fd if fd == reconnect_fd => {
                self.handle_reconnect_event(ops);
            }
            _ => error!("upcall epoll handler: unknown event"),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!("UpcallEpollHandler: init");
        // add the reconnect time fd into epoll manager
        ops.add(Events::new(&self.reconnect_timer, EventSet::IN))
            .unwrap();
        // add the first stream into epoll manager
        let info = self.info.lock().unwrap();
        ops.add(Events::new_raw(
            info.stream.as_ref().unwrap().as_raw_fd(),
            EventSet::IN,
        ))
        .unwrap();
    }
}

/// The definition of upcall client service.
pub trait UpcallClientService {
    /// Start to connect to service.
    fn connection_start(&self, stream: &mut Box<dyn VsockStream>) -> Result<()>;
    /// Check service's connection callback.
    fn connection_check(&self, stream: &mut Box<dyn VsockStream>) -> Result<()>;
    /// Send request to service.
    fn send_request(
        &self,
        stream: &mut Box<dyn VsockStream>,
        request: UpcallClientRequest,
    ) -> Result<()>;
    /// Service's response callback.
    fn handle_response(&self, stream: &mut Box<dyn VsockStream>) -> Result<UpcallClientResponse>;
}

#[cfg(test)]
mod tests {
    use dbs_utils::epoll_manager::SubscriberOps;
    use dbs_virtio_devices::vsock::backend::{VsockBackend, VsockInnerBackend};

    use super::*;

    #[derive(Default)]
    struct FakeService {
        connection_start_err: bool,
        connection_check_err: bool,
        handle_response_err: bool,
    }

    impl UpcallClientService for FakeService {
        fn connection_start(&self, stream: &mut Box<dyn VsockStream>) -> Result<()> {
            if self.connection_start_err {
                return Err(UpcallClientError::InvalidMessage(String::from(
                    "test failed",
                )));
            }
            stream
                .write_all(&String::from("CONN START").into_bytes())
                .unwrap();
            Ok(())
        }
        fn connection_check(&self, stream: &mut Box<dyn VsockStream>) -> Result<()> {
            if self.connection_check_err {
                return Err(UpcallClientError::InvalidMessage(String::from(
                    "test failed",
                )));
            }
            let mut buffer = [0; 10];
            stream.read_exact(&mut buffer).unwrap();
            assert_eq!(buffer, String::from("CONN CHECK").into_bytes().as_slice());
            Ok(())
        }
        fn send_request(
            &self,
            stream: &mut Box<dyn VsockStream>,
            _request: UpcallClientRequest,
        ) -> Result<()> {
            stream
                .write_all(&String::from("TEST REQ").into_bytes())
                .unwrap();
            Ok(())
        }

        fn handle_response(
            &self,
            stream: &mut Box<dyn VsockStream>,
        ) -> Result<UpcallClientResponse> {
            if self.handle_response_err {
                return Err(UpcallClientError::InvalidMessage(String::from(
                    "test failed",
                )));
            }
            let mut buffer = [0; 9];
            stream.read_exact(&mut buffer).unwrap();
            assert_eq!(buffer, String::from("TEST RESP").into_bytes().as_slice());
            Ok(UpcallClientResponse::FakeResponse)
        }
    }

    fn get_upcall_client_info() -> (VsockInnerBackend, UpcallClientInfo<FakeService>) {
        let vsock_backend = VsockInnerBackend::new().unwrap();
        let connector = vsock_backend.get_connector();
        let upcall_client_info = UpcallClientInfo {
            service: FakeService::default(),
            connector,
            stream: None,
            state: UpcallClientState::WaitingServer,
            result_callback: None,
        };
        (vsock_backend, upcall_client_info)
    }

    #[test]
    fn test_upcall_client_info_server_connection_start_and_check() {
        let (mut vsock_backend, mut info) = get_upcall_client_info();

        assert!(info.server_connection_start().is_ok());
        assert!(info.stream.is_some());

        let mut inner_stream = vsock_backend.accept().unwrap();
        let mut read_buffer = vec![0; 12];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());
        assert_eq!(
            read_buffer,
            format!("CONNECT {SERVER_PORT}\n",).into_bytes()
        );

        let writer_buffer = String::from("ERR").into_bytes();
        inner_stream.write_all(&writer_buffer).unwrap();
        assert!(info.server_connection_check().is_err());

        let writer_buffer = String::from("OK 1024\n").into_bytes();
        inner_stream.write_all(&writer_buffer).unwrap();
        assert!(info.server_connection_check().is_ok());
    }

    #[test]
    fn test_upcall_client_info_service_connection() {
        let (mut vsock_backend, mut info) = get_upcall_client_info();
        info.server_connection_start().unwrap();

        let mut inner_stream = vsock_backend.accept().unwrap();
        let mut read_buffer = vec![0; 12];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

        assert!(info.service_connection_start().is_ok());
        let mut read_buffer = vec![0; 10];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());
        assert_eq!(
            read_buffer,
            String::from("CONN START").into_bytes().as_slice()
        );

        let writer_buffer = String::from("CONN CHECK").into_bytes();
        inner_stream.write_all(&writer_buffer).unwrap();
        assert!(info.service_connection_check().is_ok());
    }

    #[test]
    fn test_upcall_client_info_request_and_response() {
        let (mut vsock_backend, mut info) = get_upcall_client_info();
        info.server_connection_start().unwrap();

        let mut inner_stream = vsock_backend.accept().unwrap();
        let mut read_buffer = vec![0; 12];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

        assert!(info.send_request(UpcallClientRequest::FakeRequest).is_ok());
        let mut read_buffer = vec![0; 8];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());
        assert_eq!(
            read_buffer,
            String::from("TEST REQ").into_bytes().as_slice()
        );

        let writer_buffer = String::from("TEST RESP").into_bytes();
        inner_stream.write_all(&writer_buffer).unwrap();
        assert!(info.handle_response().is_ok());
    }

    #[test]
    fn test_upcall_client_info_set_state() {
        let (_, mut info) = get_upcall_client_info();

        info.set_state(UpcallClientState::WaitingServer);
        assert_eq!(info.state, UpcallClientState::WaitingServer);

        info.set_state(UpcallClientState::ReconnectError);
        assert_eq!(info.state, UpcallClientState::ReconnectError);
    }

    #[test]
    fn test_upcall_client_info_callback() {
        let (_, mut info) = get_upcall_client_info();
        assert!(info.result_callback.is_none());

        let callbacked = Arc::new(Mutex::new(None));
        let callbacked_ = callbacked.clone();
        info.set_callback(Box::new(move |resp| {
            *callbacked_.lock().unwrap() = Some(resp);
        }));
        assert!(info.result_callback.is_some());

        info.consume_callback(UpcallClientResponse::FakeResponse);
        assert!(info.result_callback.is_none());
        assert_eq!(
            *callbacked.lock().unwrap(),
            Some(UpcallClientResponse::FakeResponse)
        );
    }

    fn get_upcall_client() -> (VsockInnerBackend, UpcallClient<FakeService>) {
        let vsock_backend = VsockInnerBackend::new().unwrap();
        let connector = vsock_backend.get_connector();
        let epoll_manager = EpollManager::default();
        let upcall_client =
            UpcallClient::new(connector, epoll_manager, FakeService::default()).unwrap();

        (vsock_backend, upcall_client)
    }

    #[test]
    fn test_upcall_client_connect() {
        let (mut vsock_backend, mut upcall_client) = get_upcall_client();

        assert!(upcall_client.connect().is_ok());

        let mut inner_stream = vsock_backend.accept().unwrap();
        let mut read_buffer = vec![0; 12];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());
        assert_eq!(read_buffer, format!("CONNECT {SERVER_PORT}\n").into_bytes());
    }

    #[allow(clippy::mutex_atomic)]
    #[allow(clippy::redundant_clone)]
    #[test]
    fn test_upcall_client_send_request() {
        let (mut vsock_backend, upcall_client) = get_upcall_client();
        let info = upcall_client.info.clone();
        let connector = vsock_backend.get_connector();
        let outer_stream = connector.connect().unwrap();
        info.lock().unwrap().stream = Some(outer_stream);
        let mut inner_stream = vsock_backend.accept().unwrap();

        let got_response = Arc::new(Mutex::new(false));
        // assume service is connected
        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::ServiceConnected);
        }

        let got_response_ = got_response.clone();
        assert!(upcall_client
            .send_request(
                UpcallClientRequest::FakeRequest,
                Box::new(move |_| {
                    *got_response_.lock().unwrap() = true;
                }),
            )
            .is_ok());
        assert!(info.lock().unwrap().result_callback.is_some());

        let mut read_buffer = vec![0; 8];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

        let writer_buffer = String::from("TEST RESP").into_bytes();
        assert!(inner_stream.write_all(writer_buffer.as_slice()).is_ok());
        let response = info.lock().unwrap().handle_response().unwrap();
        info.lock().unwrap().consume_callback(response);
        assert!(info.lock().unwrap().result_callback.is_none());

        assert!(*got_response.lock().unwrap());
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_upcall_client_send_request_without_result() {
        let (mut vsock_backend, upcall_client) = get_upcall_client();
        let info = upcall_client.info.clone();
        let connector = vsock_backend.get_connector();
        let outer_stream = connector.connect().unwrap();
        info.lock().unwrap().stream = Some(outer_stream);
        let mut inner_stream = vsock_backend.accept().unwrap();

        // assume service is connected
        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::ServiceConnected);
        }

        assert!(upcall_client
            .send_request_without_result(UpcallClientRequest::FakeRequest)
            .is_ok());
        assert!(info.lock().unwrap().result_callback.is_none());

        let mut read_buffer = vec![0; 8];
        assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

        let writer_buffer = String::from("TEST RESP").into_bytes();
        assert!(inner_stream.write_all(writer_buffer.as_slice()).is_ok());
        assert!(info.lock().unwrap().handle_response().is_ok());
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_upcall_client_send_request_error() {
        let (_, upcall_client) = get_upcall_client();
        let info = upcall_client.info.clone();

        let do_test = || {
            assert!(upcall_client
                .send_request_inner(UpcallClientRequest::FakeRequest, None)
                .is_err());
        };

        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::WaitingServer);
        }
        do_test();

        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::WaitingService);
        }
        do_test();

        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::ReconnectError);
        }
        do_test();

        {
            let mut i = info.lock().unwrap();
            i.set_state(UpcallClientState::ServiceBusy);
        }
        do_test();
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_upcall_client_get_state() {
        let (_, upcall_client) = get_upcall_client();

        assert_eq!(upcall_client.get_state(), UpcallClientState::WaitingServer);

        let info = upcall_client.info.clone();
        info.lock().unwrap().state = UpcallClientState::ServiceBusy;
        assert_eq!(upcall_client.get_state(), UpcallClientState::ServiceBusy);
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_upcall_client_is_ready() {
        let (_, upcall_client) = get_upcall_client();

        assert!(!upcall_client.is_ready());

        let info = upcall_client.info.clone();
        info.lock().unwrap().state = UpcallClientState::ServiceConnected;
        assert!(upcall_client.is_ready());
    }

    fn get_upcall_epoll_handler() -> (VsockInnerBackend, UpcallEpollHandler<FakeService>) {
        let (inner_backend, info) = get_upcall_client_info();
        let epoll_handler = UpcallEpollHandler::new(Arc::new(Mutex::new(info))).unwrap();

        (inner_backend, epoll_handler)
    }

    #[test]
    fn test_upcall_epoll_handler_set_reconnect() {
        let (_, mut epoll_handler) = get_upcall_epoll_handler();

        assert!(epoll_handler.set_reconnect().is_ok());
        assert_eq!(epoll_handler.reconnect_time, 1);
        assert!(epoll_handler.in_reconnect);
        match epoll_handler.reconnect_timer.get_state() {
            TimerState::Oneshot(dur) => {
                assert!(dur.as_millis() < 10 && dur.as_millis() > 5);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_upcall_epoll_handler_stream_event() {
        // Waiting Server state, server connection check error
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::WaitingServer);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(info.lock().unwrap().state, UpcallClientState::WaitingServer);
            assert_eq!(epoll_handler.reconnect_time, 1);
            assert!(epoll_handler.in_reconnect);
        }

        // Waiting Server state, server connection check success, but service
        // connection start error
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::WaitingServer);
            info.lock().unwrap().service.connection_start_err = true;

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            let writer_buffer = String::from("OK 1024\n").into_bytes();
            inner_stream.write_all(&writer_buffer).unwrap();

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(info.lock().unwrap().state, UpcallClientState::WaitingServer);
            assert_eq!(epoll_handler.reconnect_time, 1);
            assert!(epoll_handler.in_reconnect);
        }

        // Waiting Server state, server connection check success, and service
        // connection start success, too
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::WaitingServer);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            let writer_buffer = String::from("OK 1024\n").into_bytes();
            inner_stream.write_all(&writer_buffer).unwrap();

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(
                info.lock().unwrap().state,
                UpcallClientState::WaitingService
            );
        }

        // Waiting Service state, service connection check error
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::WaitingService);
            info.lock().unwrap().service.connection_check_err = true;

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(info.lock().unwrap().state, UpcallClientState::WaitingServer);
            assert_eq!(epoll_handler.reconnect_time, 1);
            assert!(epoll_handler.in_reconnect);
        }

        // Waiting Service state, service connection check ok
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::WaitingService);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            let writer_buffer = String::from("CONN CHECK").into_bytes();
            inner_stream.write_all(&writer_buffer).unwrap();

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(
                info.lock().unwrap().state,
                UpcallClientState::ServiceConnected
            );
        }

        // Service Busy state, handle response err
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::ServiceBusy);
            info.lock().unwrap().service.handle_response_err = true;

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(info.lock().unwrap().state, UpcallClientState::WaitingServer);
            assert_eq!(epoll_handler.reconnect_time, 1);
            assert!(epoll_handler.in_reconnect);
        }

        // Service Busy state, handle response ok
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::ServiceBusy);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            let writer_buffer = String::from("TEST RESP").into_bytes();
            inner_stream.write_all(&writer_buffer).unwrap();

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(
                info.lock().unwrap().state,
                UpcallClientState::ServiceConnected
            );
        }

        // Service Connected state
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::ServiceConnected);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(
                info.lock().unwrap().state,
                UpcallClientState::ServiceConnected
            );
        }

        // Reconnect Error state
        {
            let (_, epoll_handler) = get_upcall_epoll_handler();
            let mgr = EpollManager::default();
            let id = mgr.add_subscriber(Box::new(epoll_handler));
            let mut inner_mgr = mgr.mgr.lock().unwrap();
            let mut event_ops = inner_mgr.event_ops(id).unwrap();
            let (mut vsock_backend, mut epoll_handler) = get_upcall_epoll_handler();
            let info = epoll_handler.info.clone();
            let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
            event_ops
                .add(Events::new_raw(stream_fd, EventSet::IN))
                .unwrap();

            let info = epoll_handler.info.clone();
            info.lock()
                .unwrap()
                .set_state(UpcallClientState::ReconnectError);

            let mut inner_stream = vsock_backend.accept().unwrap();
            let mut read_buffer = vec![0; 12];
            assert!(inner_stream.read_exact(&mut read_buffer).is_ok());

            epoll_handler.handle_stream_event(&mut event_ops);
            assert_eq!(
                info.lock().unwrap().state,
                UpcallClientState::ReconnectError
            );
        }
    }

    #[test]
    fn test_upcall_epoll_handler_reconnect_event() {
        let (_, epoll_handler) = get_upcall_epoll_handler();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(epoll_handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_ops = inner_mgr.event_ops(id).unwrap();
        let (_, mut epoll_handler) = get_upcall_epoll_handler();

        epoll_handler.handle_reconnect_event(&mut event_ops);
    }

    #[test]
    fn test_upcall_epoll_handler_process() {
        let (_, epoll_handler) = get_upcall_epoll_handler();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(epoll_handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_ops = inner_mgr.event_ops(id).unwrap();
        let (_, mut epoll_handler) = get_upcall_epoll_handler();
        let info = epoll_handler.info.clone();
        let stream_fd = info.lock().unwrap().stream.as_ref().unwrap().as_raw_fd();
        let reconnect_fd = epoll_handler.reconnect_timer.as_raw_fd();
        let event_set = EventSet::EDGE_TRIGGERED;
        event_ops
            .add(Events::new_raw(stream_fd, EventSet::IN))
            .unwrap();

        // test for stream event
        let events = Events::new_raw(stream_fd, event_set);
        epoll_handler.process(events, &mut event_ops);

        // test for reconnect event
        let events = Events::new_raw(reconnect_fd, event_set);
        epoll_handler.process(events, &mut event_ops);
    }
}
