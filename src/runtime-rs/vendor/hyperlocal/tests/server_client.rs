use std::{error::Error, fs, path::Path};

use hyper::{
    body::HttpBody,
    service::{make_service_fn, service_fn},
    Body, Client, Response, Server,
};
use hyperlocal::{UnixClientExt, UnixServerExt, Uri};

#[tokio::test]
async fn test_server_client() -> Result<(), Box<dyn Error + Send + Sync>> {
    let path = Path::new("/tmp/hyperlocal.sock");

    if path.exists() {
        fs::remove_file(path)?;
    }

    let make_service = make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(|_req| async {
            Ok::<_, hyper::Error>(Response::new(Body::from("It works!")))
        }))
    });

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let server = Server::bind_unix("/tmp/hyperlocal.sock")?
        .serve(make_service)
        .with_graceful_shutdown(async { rx.await.unwrap() });

    tokio::spawn(async move { server.await.unwrap() });

    let client = Client::unix();

    let url = Uri::new(path, "/").into();

    let mut response = client.get(url).await?;
    let mut bytes = Vec::default();

    while let Some(next) = response.data().await {
        let chunk = next?;
        bytes.extend(chunk);
    }

    let string = String::from_utf8(bytes)?;

    tx.send(()).unwrap();

    assert_eq!(string, "It works!");

    Ok(())
}
