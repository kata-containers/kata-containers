use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;
use tokio_io_timeout::TimeoutStream;

use hyper::client::connect::{Connected, Connection};
use hyper::{service::Service, Uri};

mod stream;

use stream::TimeoutConnectorStream;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A connector that enforces as connection timeout
#[derive(Debug, Clone)]
pub struct TimeoutConnector<T> {
    /// A connector implementing the `Connect` trait
    connector: T,
    /// Amount of time to wait connecting
    connect_timeout: Option<Duration>,
    /// Amount of time to wait reading response
    read_timeout: Option<Duration>,
    /// Amount of time to wait writing request
    write_timeout: Option<Duration>,
}

impl<T> TimeoutConnector<T>
where
    T: Service<Uri> + Send,
    T::Response: AsyncRead + AsyncWrite + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    /// Construct a new TimeoutConnector with a given connector implementing the `Connect` trait
    pub fn new(connector: T) -> Self {
        TimeoutConnector {
            connector,
            connect_timeout: None,
            read_timeout: None,
            write_timeout: None,
        }
    }
}

impl<T> Service<Uri> for TimeoutConnector<T>
where
    T: Service<Uri> + Send,
    T::Response: AsyncRead + AsyncWrite + Connection + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    type Response = Pin<Box<TimeoutConnectorStream<T::Response>>>;
    type Error = BoxError;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connector.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let connect_timeout = self.connect_timeout;
        let read_timeout = self.read_timeout;
        let write_timeout = self.write_timeout;
        let connecting = self.connector.call(dst);

        let fut = async move {
            let stream = match connect_timeout {
                None => {
                    let io = connecting.await.map_err(Into::into)?;
                    TimeoutStream::new(io)
                }
                Some(connect_timeout) => {
                    let timeout = timeout(connect_timeout, connecting);
                    let connecting = timeout
                        .await
                        .map_err(|e| io::Error::new(io::ErrorKind::TimedOut, e))?;
                    let io = connecting.map_err(Into::into)?;
                    TimeoutStream::new(io)
                }
            };

            let mut tm = TimeoutConnectorStream::new(stream);
            tm.set_read_timeout(read_timeout);
            tm.set_write_timeout(write_timeout);
            Ok(Box::pin(tm))
        };

        Box::pin(fut)
    }
}

impl<T> TimeoutConnector<T> {
    /// Set the timeout for connecting to a URL.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_connect_timeout(&mut self, val: Option<Duration>) {
        self.connect_timeout = val;
    }

    /// Set the timeout for the response.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_read_timeout(&mut self, val: Option<Duration>) {
        self.read_timeout = val;
    }

    /// Set the timeout for the request.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_write_timeout(&mut self, val: Option<Duration>) {
        self.write_timeout = val;
    }
}

impl<T> Connection for TimeoutConnector<T>
where
    T: AsyncRead + AsyncWrite + Connection + Service<Uri> + Send + Unpin,
    T::Response: AsyncRead + AsyncWrite + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    fn connected(&self) -> Connected {
        self.connector.connected()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::time::Duration;

    use hyper::client::HttpConnector;
    use hyper::Client;

    use super::TimeoutConnector;

    #[tokio::test]
    async fn test_timeout_connector() {
        // 10.255.255.1 is a not a routable IP address
        let url = "http://10.255.255.1".parse().unwrap();

        let http = HttpConnector::new();
        let mut connector = TimeoutConnector::new(http);
        connector.set_connect_timeout(Some(Duration::from_millis(1)));

        let client = Client::builder().build::<_, hyper::Body>(connector);

        let res = client.get(url).await;

        match res {
            Ok(_) => panic!("Expected a timeout"),
            Err(e) => {
                if let Some(io_e) = e.source().unwrap().downcast_ref::<io::Error>() {
                    assert_eq!(io_e.kind(), io::ErrorKind::TimedOut);
                } else {
                    panic!("Expected timeout error");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_read_timeout() {
        let url = "http://example.com".parse().unwrap();

        let http = HttpConnector::new();
        let mut connector = TimeoutConnector::new(http);
        // A 1 ms read timeout should be so short that we trigger a timeout error
        connector.set_read_timeout(Some(Duration::from_millis(1)));

        let client = Client::builder().build::<_, hyper::Body>(connector);

        let res = client.get(url).await;

        match res {
            Ok(_) => panic!("Expected a timeout"),
            Err(e) => {
                if let Some(io_e) = e.source().unwrap().downcast_ref::<io::Error>() {
                    assert_eq!(io_e.kind(), io::ErrorKind::TimedOut);
                } else {
                    panic!("Expected timeout error");
                }
            }
        }
    }
}
