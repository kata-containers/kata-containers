// Copyright (c) 2022 Alibaba Cloud
// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Shim management service, this service starts a management http server on a socket
// and wire certain URL with a corresponding handler. When a command-line interface
// or further shim functions want the information corresponding to this, it can just
// send a GET request to the url, and the info will be in the response

#![allow(dead_code)] // some url's handler are *to be* developed

use std::{convert::Infallible, fs, future::Future, path::Path, sync::Arc};

use anyhow::{Context, Result};
use bytes::Bytes;
use common::Sandbox;
use http_body_util::Full;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use shim_interface::{sb_storage_path, SHIM_MGMT_SOCK_NAME};
use tokio::net::{UnixListener, UnixStream};

use super::handlers::handler_mux;

/// The shim management server instance
pub struct MgmtServer {
    /// socket address(with prefix like hvsock://)
    pub s_addr: String,

    /// The sandbox instance
    pub sandbox: Arc<dyn Sandbox>,
}

impl MgmtServer {
    /// construct a new management server
    pub fn new(sid: &str, sandbox: Arc<dyn Sandbox>) -> Result<Self> {
        // make sure the storage path exists, and the socket file will be created in that path
        let kata_path = sb_storage_path();
        fs::create_dir_all(kata_path)
            .context(format!("failed to create kata path directory {kata_path}"))?;

        let s_addr = format!("unix://{kata_path}/{sid}/{SHIM_MGMT_SOCK_NAME}");

        Ok(Self { s_addr, sandbox })
    }

    // TODO(when metrics is supported): write metric addresses to fs
    // TODO(when metrics is supported): register shim metrics
    // TODO(when metrics is supported): register sandbox metrics
    // running management http server in an infinite loop, able to serve concurrent requests
    pub async fn run(self: Arc<Self>) {
        let listener = listener_from_path(self.s_addr.clone()).await.unwrap();
        // start an infinite loop, which serves the incomming uds stream
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let me = self.clone();
            // spawn a light weight thread to multiplex to the handler
            tokio::task::spawn(async move {
                if let Err(err) = serve_management_connection(stream, |request| {
                    handler_mux(me.sandbox.clone(), request)
                })
                .await
                {
                    warn!(sl!(), "Failed to serve connection: {:?}", err);
                }
            });
        }
    }
}

async fn serve_management_connection<H, F>(
    stream: UnixStream,
    handler: H,
) -> Result<(), hyper::Error>
where
    H: Fn(Request<Incoming>) -> F,
    F: Future<Output = Result<Response<Full<Bytes>>>>,
{
    http1::Builder::new()
        .serve_connection(
            TokioIo::new(stream),
            service_fn(move |request| {
                let result = handler(request);
                async move { handler_result_to_response(result.await) }
            }),
        )
        .await
}

// Returning a service error would close the connection without an HTTP response.
fn handler_result_to_response(
    result: Result<Response<Full<Bytes>>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(match result {
        Ok(response) => response,
        Err(error) => {
            warn!(sl!(), "Management request failed"; "error" => format!("{error:#}"));
            let mut response =
                Response::new(Full::new(Bytes::from_static(b"Internal server error\n")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            response
        }
    })
}

// from path, return a unix listener corresponding to that path,
// if the path(socket file) is not created, we create that here
async fn listener_from_path(path: String) -> Result<UnixListener> {
    // create the socket if not present
    let trim_path = path.strip_prefix("unix:").context("trim path")?;
    let file_path = Path::new("/").join(trim_path);
    let file_path = file_path.as_path();
    if let Some(parent_dir) = file_path.parent() {
        fs::create_dir_all(parent_dir).context("create parent dir")?;
    }
    // bind the socket and return the listener
    info!(sl!(), "mgmt-svr: binding to path {}", path);
    UnixListener::bind(file_path).context("bind address")
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use http_body_util::{BodyExt, Empty};
    use hyper::client::conn::http1 as client_http1;
    use hyper::header::{CONNECTION, HOST};
    use rstest::rstest;
    use std::time::Duration;
    use tokio::task::JoinSet;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_handler_error_response() {
        let error = anyhow!("sensitive guest policy details").context("Failed to set iptable");
        let response = handler_result_to_response(Err(error)).unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()[CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            Bytes::from_static(b"Internal server error\n")
        );
    }

    #[rstest]
    #[case::binary_success(StatusCode::CREATED, b"\x00\xffiptables\n")]
    #[case::empty_success(StatusCode::OK, b"")]
    #[case::not_found(StatusCode::NOT_FOUND, b"URL NOT FOUND")]
    #[case::explicit_error(StatusCode::INTERNAL_SERVER_ERROR, b"existing error response")]
    #[tokio::test]
    async fn test_handler_response_is_unchanged(
        #[case] status: StatusCode,
        #[case] body: &'static [u8],
    ) {
        let body = Bytes::from_static(body);
        let response = Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header("x-management-test", "preserved")
            .body(Full::new(body.clone()))
            .unwrap();
        let response = handler_result_to_response(Ok(response)).unwrap();

        assert_eq!(response.status(), status);
        assert_eq!(response.headers()[CONTENT_TYPE], "application/octet-stream");
        assert_eq!(response.headers()["x-management-test"], "preserved");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            body
        );
    }

    #[tokio::test]
    async fn test_handler_error_over_unix_socket() {
        let request = Request::builder()
            .uri("/iptables")
            .header(HOST, "localhost")
            .header(CONNECTION, "close")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let (server_stream, client_stream) = UnixStream::pair().unwrap();
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            serve_management_connection(server_stream, |_request| async {
                Err(anyhow!("sensitive agent RPC error").context("Failed to get iptable"))
            })
            .await
        });

        let result = timeout(Duration::from_secs(5), async {
            let (mut client, connection) =
                client_http1::handshake(TokioIo::new(client_stream)).await?;
            tasks.spawn(connection);
            let response = client.send_request(request).await?;
            let status = response.status();
            let body = response.into_body().collect().await?.to_bytes();
            Ok::<_, hyper::Error>((status, body))
        })
        .await;
        tasks.shutdown().await;

        let (status, body) = result
            .expect("management request timed out")
            .expect("management server closed the connection without an HTTP response");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, Bytes::from_static(b"Internal server error\n"));
    }
}
