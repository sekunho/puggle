use std::{convert::Infallible, net::SocketAddr};
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1,
    Request, Response,
};
use http_body_util::Full;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use std::{
    path::Path,
    sync::Arc,
};

use hyper_util::rt::TokioTimer;
use puggle_lib::Config;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind tcp listener to port. reason: {0}")]
    TcpListener(#[from] std::io::Error),
}

pub async fn run(config: &Config) -> Result<(), ServerError> {
    let local_address = SocketAddr::from(([0, 0, 0, 0], config.preview.port));
    let listener = TcpListener::bind(local_address).await?;

    let template_handle = Arc::from(puggle_lib::init_template_handle(
        config.templates_dir.clone(),
        config.dest_dir.clone(),
    ));

    let mut build_notifier = puggle_notifier::Handle::new().unwrap();
    let mut dest_dir_notifier = puggle_notifier::Handle::new().unwrap();

    let _ = tokio::join!(
        build_notifier.watch(Path::new("blog")),
        dest_dir_notifier.watch(config.dest_dir.as_path()),
        execute_server(template_handle, listener),
    );

    Ok(())
}

async fn execute_server(
    template_handle: Arc<template::Handle>,
    listener: TcpListener,
) -> Result<(), ServerError> {
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let template_handle = Arc::clone(&template_handle);

        tokio::task::spawn(async move {
            let svc = hyper_util::service::TowerToHyperService::new(
                tower_http::services::ServeDir::new("public")
            );

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

async fn hello(_: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}
