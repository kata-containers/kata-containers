use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;

async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("Hello World")))
}

fn main() {
    actix_rt::System::with_tokio_rt(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    })
    .block_on(async {
        let make_service =
            make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

        let server =
            Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000))).serve(make_service);

        if let Err(err) = server.await {
            eprintln!("server error: {}", err);
        }
    })
}
