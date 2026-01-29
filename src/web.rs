use axum::{
    routing::{get, post},
    extract::{State, Query, Json},
    response::{IntoResponse, Html},
    http::{StatusCode, Uri},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::config::Config;
use crate::trie::Trie;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use rust_embed::RustEmbed;
use arboard::Clipboard;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub enum ClipboardRequest {
    SetText(String),
}

pub struct WebServer {
    pub port: u16,
    pub config: Arc<RwLock<Config>>,
    pub tries: Arc<RwLock<HashMap<String, Trie>>>,
    pub clipboard_tx: UnboundedSender<ClipboardRequest>,
    pub tray_tx: std::sync::mpsc::Sender<crate::tray::TrayEvent>,
}

type WebState = (
    Arc<RwLock<Config>>, 
    Arc<RwLock<HashMap<String, Trie>>>, 
    UnboundedSender<ClipboardRequest>,
    std::sync::mpsc::Sender<crate::tray::TrayEvent>
);

impl WebServer {
    pub fn new(
        port: u16, 
        config: Arc<RwLock<Config>>, 
        tries: Arc<RwLock<HashMap<String, Trie>>>,
        tray_tx: std::sync::mpsc::Sender<crate::tray::TrayEvent>
    ) -> Self {
        let (tx, mut rx) = unbounded_channel::<ClipboardRequest>();

        std::thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(cb) => Some(cb),
                Err(e) => {
                    eprintln!("[Web] Warning: Failed to initialize system clipboard: {}", e);
                    None
                }
            };

            while let Some(req) = rx.blocking_recv() {
                match req {
                    ClipboardRequest::SetText(text) => {
                        if let Some(cb) = clipboard.as_mut() {
                            let _ = cb.set_text(text);
                        }
                    }
                }
            }
        });

        Self { 
            port, 
            config, 
            tries,
            clipboard_tx: tx,
            tray_tx,
        }
    }

    pub async fn start(self) {
        let state: WebState = (self.config, self.tries, self.clipboard_tx, self.tray_tx);
        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config).post(update_config))
            .route("/api/convert", get(convert_handler))
            .route("/api/dicts", get(list_dicts))
            .route("/api/dict/content", get(get_dict_content).post(save_dict_content))
            .route("/api/dicts/reload", post(reload_dicts))
            .route("/static/*file", get(static_handler))
            .fallback(index_handler)
            .with_state(state);

        let addr = format!("127.0.0.1:{}", self.port);
        println!("[Web] 服务器启动在 http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
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

async fn get_config(State((config, _, _, _)): State<WebState>) -> Json<Config> {
    Json(config.read().unwrap().clone())
}

async fn update_config(
    State((config, _, _, tray_tx)): State<WebState>,
    Json(new_config): Json<Config>
) -> StatusCode {
    {
        let mut w = config.write().unwrap();
        *w = new_config.clone();
    }

    if let Err(_e) = crate::save_config(&new_config) {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    let _ = tray_tx.send(crate::tray::TrayEvent::ReloadConfig);
    StatusCode::OK
}

#[derive(Deserialize)]
struct ConvertParams {
    text: String,
}

async fn convert_handler(
    State((config, tries, clipboard_tx, _)): State<WebState>,
    Query(params): Query<ConvertParams>,
) -> String {
    let c = config.read().unwrap();
    let t = tries.read().unwrap();
    let active_profile = &c.input.default_profile;

    let dict = if let Some(d) = t.get(active_profile) { d } else { return params.text; };

    let mut result = String::new();
    let chars: Vec<char> = params.text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if !chars[i].is_ascii_alphabetic() {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        let mut found = false;
        for len in (1..=(chars.len() - i).min(15)).rev() {
            let sub: String = chars[i..i+len].iter().collect();
            let sub_lower = sub.to_lowercase();
            if let Some((word_match, _hint)) = dict.get_all_exact(&sub_lower).and_then(|v| v.first().cloned()) {
                result.push_str(&word_match);
                i += len;
                found = true;
                break;
            }
        }

        if !found {
            result.push(chars[i]);
            i += 1;
        }
    }

    let _ = clipboard_tx.send(ClipboardRequest::SetText(result.clone()));
    result
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
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    list.push(DictFile {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: entry.path().to_string_lossy().to_string(),
                        size: meta.len(),
                    });
                }
            }
        }
    }
    Json(list)
}

async fn get_dict_content() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

async fn save_dict_content() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

async fn reload_dicts(State((_, _, _, tray_tx)): State<WebState>) -> StatusCode {
    let _ = tray_tx.send(crate::tray::TrayEvent::ReloadConfig);
    StatusCode::OK
}