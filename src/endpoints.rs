use crate::service::*;
use axum::response::*;
use axum::Json;
use axum::{extract::DefaultBodyLimit, extract::Extension, routing::*, Router, Server};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::trace::*;
use tracing::*;

pub async fn handle_start(
    Extension(shared_state): Extension<Arc<Mutex<RecordingState>>>,
    Json(opt): Json<RecordingOptions>,
) -> impl IntoResponse {
    let mx = shared_state.clone();
    tokio::spawn(start(mx, opt));
    Json("STARTED")
}

pub async fn handle_status(
    Extension(state): Extension<Arc<Mutex<RecordingState>>>,
) -> impl IntoResponse {
    let mx = state.clone();
    let s = mx.lock().await.clone();
    Json(s).into_response()
}

pub async fn handle_stop(
    Extension(shared_state): Extension<Arc<Mutex<RecordingState>>>,
) -> impl IntoResponse {
    let mx = shared_state.clone();
    tokio::spawn(stop(mx));
    Json("STOPPED")
}

use std::net::SocketAddr;

pub async fn run(socket_addr: SocketAddr, public_dir: &str) -> anyhow::Result<()> {
    let serve_dir = ServeDir::new(public_dir);
    let shared_state = Arc::new(Mutex::new(RecordingState::Waiting));
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let app = Router::new()
        .nest_service("/", serve_dir.clone())
        .route("/api/start", post(handle_start))
        .route("/api/stop", post(handle_stop))
        .route("/api/status", get(handle_status))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(1 * 1024 * 1024))
        .layer(Extension(shared_state))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::DEBUG))
                .on_request(DefaultOnRequest::new().level(Level::TRACE))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true),
                ),
        )
        .layer(cors);

    info!("Server is listening on {}", socket_addr);
    Server::bind(&socket_addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}
