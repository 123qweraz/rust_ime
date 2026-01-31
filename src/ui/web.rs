use axum::{
    routing::{get, post},
    extract::{State, Json},
    response::{IntoResponse, Html},
    http::{StatusCode, Uri},
    Router,
};
use serde::{Serialize};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::config::Config;
use crate::engine::trie::Trie;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub struct WebServer {
    pub port: u16,
    pub config: Arc<RwLock<Config>>,
    pub tries: Arc<RwLock<HashMap<String, Trie>>>,
    pub tray_tx: std::sync::mpsc::Sender<crate::ui::tray::TrayEvent>,
}

type WebState = (
    Arc<RwLock<Config>>, 
    Arc<RwLock<HashMap<String, Trie>>>, 
    std::sync::mpsc::Sender<crate::ui::tray::TrayEvent>
);

impl WebServer {
    pub fn new(
        port: u16, 
        config: Arc<RwLock<Config>>, 
        tries: Arc<RwLock<HashMap<String, Trie>>>,
        tray_tx: std::sync::mpsc::Sender<crate::ui::tray::TrayEvent>
    ) -> Self {
        Self { port, config, tries, tray_tx }
    }

    pub async fn start(self) {
        let state: WebState = (self.config, self.tries, self.tray_tx);
        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config).post(update_config))
            .route("/api/config/reset", post(reset_config))
            .route("/api/dicts", get(list_dicts))
            .route("/api/dicts/compile", post(compile_dicts_handler))
            .route("/api/dicts/reload", post(reload_dicts))
            .route("/static/*file", get(static_handler))
            .fallback(index_handler)
            .with_state(state);

        let addr = format!("127.0.0.1:{}", self.port);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                println!("[Web] 服务器启动在 http://{}", addr);
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("[Web] Server error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[Web] Failed to bind to {}: {}", addr, e);
            }
        }
    }
}

async fn index_handler() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
        None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches("/static/").trim_start_matches("/");
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

async fn get_config(State((config, _, _)): State<WebState>) -> impl IntoResponse {
    match config.read() {
        Ok(c) => Json(c.clone()).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn update_config(
    State((config, _, tray_tx)): State<WebState>,
    Json(new_config): Json<Config>
) -> StatusCode {
    {
        let mut w = match config.write() {
            Ok(w) => w,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
        };
        *w = new_config.clone();
    }
    if let Err(_e) = crate::save_config(&new_config) { return StatusCode::INTERNAL_SERVER_ERROR; }
    let _ = tray_tx.send(crate::ui::tray::TrayEvent::ReloadConfig);
    StatusCode::OK
}

async fn reset_config(
    State((config, _, tray_tx)): State<WebState>,
) -> StatusCode {
    let default_conf = Config::default_config();
    {
        let mut w = match config.write() {
            Ok(w) => w,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
        };
        *w = default_conf.clone();
    }
    if let Err(_e) = crate::save_config(&default_conf) { return StatusCode::INTERNAL_SERVER_ERROR; }
    let _ = tray_tx.send(crate::ui::tray::TrayEvent::ReloadConfig);
    StatusCode::OK
}

#[derive(Serialize)]
struct DictFile {
    name: String,
    path: String,
    size: u64,
}

async fn list_dicts() -> Json<Vec<DictFile>> {
    let mut list = Vec::new();
    if let Ok(entries) = std::fs::read_dir("dicts") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                // 递归扫描子目录
                if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                    for sub_entry in sub_entries.flatten() {
                        if sub_entry.path().extension().map_or(false, |ext| ext == "json") {
                            list.push(DictFile {
                                name: sub_entry.file_name().to_string_lossy().to_string(),
                                path: sub_entry.path().to_string_lossy().to_string(),
                                size: sub_entry.metadata().map(|m| m.len()).unwrap_or(0),
                            });
                        }
                    }
                }
            } else if entry.path().extension().map_or(false, |ext| ext == "json") {
                list.push(DictFile {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                });
            }
        }
    }
    Json(list)
}

async fn compile_dicts_handler() -> StatusCode {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("run").arg("--bin").arg("compile_dict");
    match cmd.status() {
        Ok(s) if s.success() => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn reload_dicts(State((_, _, tray_tx)): State<WebState>) -> StatusCode {
    let _ = tray_tx.send(crate::ui::tray::TrayEvent::ReloadConfig);
    StatusCode::OK
}
