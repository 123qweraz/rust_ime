use axum::{
    routing::{get, post},
    Json, Router, response::{Html, IntoResponse, Response},
    http::{StatusCode, header, Uri},
    extract::State,
};
use rust_embed::RustEmbed;
use crate::config::Config;
use crate::save_config;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub struct WebServer {
    pub port: u16,
    pub config: Arc<RwLock<Config>>,
}

impl WebServer {
    pub fn new(port: u16, config: Arc<RwLock<Config>>) -> Self {
        Self { port, config }
    }

    pub async fn start(self) {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        println!("[Web] 服务器启动在 http://{}", addr);

        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config))
            .route("/api/config", post(update_config))
            .fallback(static_handler)
            .with_state(self.config);

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

async fn index_handler() -> impl IntoResponse {
    let asset = Assets::get("index.html").unwrap();
    Html(std::str::from_utf8(&asset.data).unwrap().to_string())
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        return index_handler().await.into_response();
    }

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(axum::body::Body::from(content.data))
                .unwrap()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_config(State(config): State<Arc<RwLock<Config>>>) -> Json<Config> {
    Json(config.read().unwrap().clone())
}

async fn update_config(
    State(config): State<Arc<RwLock<Config>>>,
    Json(new_config): Json<Config>
) -> StatusCode {
    // 1. Update memory
    {
        let mut w = config.write().unwrap();
        *w = new_config.clone();
    }

    // 2. Save to file
    match save_config(&new_config) {
        Ok(_) => {
            println!("[Web] 配置已通过网页端更新并保存。");
            StatusCode::OK
        }
        Err(e) => {
            eprintln!("[Web] 保存配置失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}