// axum 0.7 style main.rs — no axum::Server
use axum::{routing::{get, post}, Router};
use axum::http::{header, Method};
use tower_http::cors::{Any, CorsLayer};
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod blender_bridge;
mod ollama; // used by blender_bridge

#[tokio::main]
async fn main() {
    // Build the REST router
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/blender/next_step", post(blender_bridge::next_step))
        .route("/blender/run_macro", post(blender_bridge::run_macro))
        .route("/blender/fix_error", post(blender_bridge::fix_error))
        .route("/blender/remember", post(blender_bridge::remember_api))
        .layer(cors);

    // Spawn the REST server on 127.0.0.1:17890
    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 17890));
        let listener = TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    // Launch the Tauri window (frontend is configured in tauri.conf.json)
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error running tauri");
}
