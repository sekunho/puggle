use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use puggle_lib::Config;
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind tcp listener to port. reason: {0}")]
    TcpListener(#[from] std::io::Error),
}

pub async fn run(config: Config) -> Result<(), ServerError> {
    let local_address = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(local_address).await?;

    let template_handle = Arc::from(puggle_lib::init_template_handle(
        config.templates_dir,
        config.dest_dir.clone(),
    ));

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let template_handle = Arc::clone(&template_handle);

        tokio::task::spawn(async move {
            let service =
                service_fn(move |_req| {
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
