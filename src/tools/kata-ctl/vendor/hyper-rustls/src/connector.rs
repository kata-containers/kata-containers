use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{fmt, io};

use hyper::{client::connect::Connection, service::Service, Uri};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

use crate::stream::MaybeHttpsStream;

pub mod builder;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A Connector for the `https` scheme.
#[derive(Clone)]
pub struct HttpsConnector<T> {
    force_https: bool,
    http: T,
    tls_config: Arc<rustls::ClientConfig>,
}

impl<T> fmt::Debug for HttpsConnector<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HttpsConnector")
            .field("force_https", &self.force_https)
            .finish()
    }
}

impl<H, C> From<(H, C)> for HttpsConnector<H>
where
    C: Into<Arc<rustls::ClientConfig>>,
{
    fn from((http, cfg): (H, C)) -> Self {
        HttpsConnector {
            force_https: false,
            http,
            tls_config: cfg.into(),
        }
    }
}

impl<T> Service<Uri> for HttpsConnector<T>
where
    T: Service<Uri>,
    T::Response: Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    type Response = MaybeHttpsStream<T::Response>;
    type Error = BoxError;

    #[allow(clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<MaybeHttpsStream<T::Response>, BoxError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.http.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        // dst.scheme() would need to derive Eq to be matchable;
        // use an if cascade instead
        if let Some(sch) = dst.scheme() {
            if sch == &http::uri::Scheme::HTTP && !self.force_https {
                let connecting_future = self.http.call(dst);

                let f = async move {
                    let tcp = connecting_future
                        .await
                        .map_err(Into::into)?;

                    Ok(MaybeHttpsStream::Http(tcp))
                };
                Box::pin(f)
            } else if sch == &http::uri::Scheme::HTTPS {
                let cfg = self.tls_config.clone();
                let hostname = dst
                    .host()
                    .unwrap_or_default()
                    .to_string();
                let connecting_future = self.http.call(dst);

                let f = async move {
                    let tcp = connecting_future
                        .await
                        .map_err(Into::into)?;
                    let connector = TlsConnector::from(cfg);
                    let dnsname = rustls::ServerName::try_from(hostname.as_str())
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid dnsname"))?;
                    let tls = connector
                        .connect(dnsname, tcp)
                        .await
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    Ok(MaybeHttpsStream::Https(tls))
                };
                Box::pin(f)
            } else {
                let err =
                    io::Error::new(io::ErrorKind::Other, format!("Unsupported scheme {}", sch));
                Box::pin(async move { Err(err.into()) })
            }
        } else {
            let err = io::Error::new(io::ErrorKind::Other, "Missing scheme");
            Box::pin(async move { Err(err.into()) })
        }
    }
}
