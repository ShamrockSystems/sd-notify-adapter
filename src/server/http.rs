use std::{
    future::IntoFuture,
    net::{IpAddr, Ipv4Addr},
};

use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension, Json, Router};
use chrono::Utc;
use tokio::{net::TcpListener, sync::mpsc::Sender};
use tokio_util::sync::CancellationToken;
use tower_http::trace::{self, TraceLayer};
use tracing::{info, span, Instrument, Level};
use valuable::Valuable;
use valuable_serde::Serializable;

use crate::{config::SharedConfiguration, error::Error, status::SharedStatus};

pub async fn server(
    token: CancellationToken,
    config: SharedConfiguration,
    status: SharedStatus,
    ready_sender: Sender<()>,
) -> Result<(), Error> {
    let span = span!(Level::INFO, "HTTP server",);

    let span_clone = span.clone();
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .layer(Extension(status))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(span_clone)
                .make_span_with(trace::DefaultMakeSpan::new().include_headers(true))
                .on_response(trace::DefaultOnResponse::new().include_headers(true)),
        );

    let span_clone = span.clone();
    let bind_future = TcpListener::bind((
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        config.read().instrument(span_clone).await.port,
    ));

    let span_clone = span.clone();
    let listener = bind_future
        .instrument(span_clone)
        .await
        .map_err(Error::HttpBindAddress)?;

    info!("HTTP server ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    let span_clone = span.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _guard = span.enter();
            token.cancelled().instrument(span_clone).await;
            info!("Shutting down HTTP server");
        })
        .into_future()
        .await
        .map_err(Error::Http)
}

macro_rules! status {
    ($p: ident, $s: expr) => {{
        let status = get_status($s).await;
        let code = if status.$p {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        (code, Json(Serializable::new(status)))
    }};
}

async fn healthz(Extension(status): Extension<SharedStatus>) -> impl IntoResponse {
    status!(healthz, status)
}

async fn livez(Extension(status): Extension<SharedStatus>) -> impl IntoResponse {
    status!(livez, status)
}

async fn readyz(Extension(status): Extension<SharedStatus>) -> impl IntoResponse {
    status!(readyz, status)
}

async fn get_status(status: SharedStatus) -> Status {
    let status = status.read().await;
    Status {
        timestamp: Utc::now().to_rfc3339(),
        healthz: status.healthz,
        livez: status.livez,
        readyz: status.readyz,
    }
}

#[derive(Valuable)]
struct Status {
    timestamp: String,
    healthz: bool,
    livez: bool,
    readyz: bool,
}
