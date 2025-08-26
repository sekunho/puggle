use std::net::SocketAddr;

use axum::Router;
use puggle_lib::Config;
use thiserror::Error;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind tcp listener to port. reason: {0}")]
    TcpListener(#[from] std::io::Error),
}

pub async fn run(config: Config) -> Result<(), ServerError> {
    let app = Router::new()
        .nest_service("/", ServeDir::new(config.dest_dir))
        .layer(tower_http::compression::CompressionLayer::new());

    let local_address = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(local_address).await?;
    let _local_address = listener.local_addr()?;

    axum::serve(listener, app).await?;
    Ok(())
}
