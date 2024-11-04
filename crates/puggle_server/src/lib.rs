use std::net::SocketAddr;

use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use puggle_lib::Config;
use thiserror::Error;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind tcp listener to port. reason: {0}")]
    TcpListener(#[from] std::io::Error),
}

pub async fn run(config: Config) -> Result<(), ServerError> {
    let local_address = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(local_address).await?;

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let huh = ServeDir::new(config.dest_dir.as_path());

        tokio::task::spawn(async move {
            let svc = ServiceBuilder::new().service(huh);
            let svc = TowerToHyperService::new(svc);

            let result = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, svc)
                .await;

            if let Err(err) = result {
                println!("oh no: {:#?}", err);
            }
        });
    }
}
