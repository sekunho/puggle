use std::{
    ffi::OsStr,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures::{channel::mpsc::Receiver, SinkExt, StreamExt};
use hyper::{server::conn::http1, service::service_fn, Response, StatusCode, Uri};
use hyper_util::rt::{TokioIo, TokioTimer};
use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind},
    Watcher,
};
use puggle_lib::Config;
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind tcp listener to port. reason: {0}")]
    TcpListener(#[from] std::io::Error),
}

pub async fn run(config: &Config) -> Result<(), ServerError> {
    let local_address = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(local_address).await?;

    let template_handle = Arc::from(puggle_lib::init_template_handle(
        config.templates_dir.clone(),
        config.dest_dir.clone(),
    ));

    let mut notifier_handle = puggle_notifier::Handle::new().unwrap();

    let _ = tokio::join!(
        notifier_handle.watch(Path::new(".")),
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
            let service =
                service_fn(move |req| {
                    let path = req.uri().path();
                    let html = template_handle.render_template_str(
                    minijinja::context!(),
                    "{% extends \"layout/base.html\" %} {% block content %}hey{% endblock %}",
                ).unwrap();

                    async move { Response::builder().status(StatusCode::NOT_FOUND).body(html) }
                });

            let result = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, service)
                .await;

            if let Err(err) = result {
                println!("oh no: {:#?}", err);
            }
        });
    }
}
