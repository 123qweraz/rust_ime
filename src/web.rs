use axum::{
    routing::{get, post},
    Json, Router, response::{Html, IntoResponse, Response},
    http::{StatusCode, header, Uri},
    extract::{State, Query},
};
use rust_embed::RustEmbed;
use crate::config::Config;
use crate::save_config;
use crate::trie::Trie;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;
use std::collections::HashMap;
use serde::Deserialize;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub struct WebServer {
    pub port: u16,
    pub config: Arc<RwLock<Config>>,
    pub tries: Arc<RwLock<HashMap<String, Trie>>>,
}

type WebState = (Arc<RwLock<Config>>, Arc<RwLock<HashMap<String, Trie>>>);

impl WebServer {
    pub fn new(port: u16, config: Arc<RwLock<Config>>, tries: Arc<RwLock<HashMap<String, Trie>>>) -> Self {
        Self { port, config, tries }
    }

    pub async fn start(self) {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        println!("[Web] 服务器启动在 http://{}", addr);

        let state: WebState = (self.config, self.tries);

        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config))
            .route("/api/config", post(update_config))
            .route("/api/convert", get(convert_handler))
            .fallback(static_handler)
            .with_state(state);

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

async fn get_config(State((config, _)): State<WebState>) -> Json<Config> {
    Json(config.read().unwrap().clone())
}

async fn update_config(
    State((config, _)): State<WebState>,
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

#[derive(Deserialize)]
struct ConvertParams {
    text: String,
}

async fn convert_handler(
    State((config, tries)): State<WebState>,
    Query(params): Query<ConvertParams>,
) -> String {
    let c = config.read().unwrap();
    let t = tries.read().unwrap();
    
    let active_profile = &c.input.default_profile;
    let dict = match t.get(active_profile) {
        Some(d) => d,
        None => return params.text, // No dict, return as is
    };

    // Use the same logic as Ime::convert_text but without the full Ime struct
    // (Actually, since we want to avoid duplication, we could make it a standalone function,
    // but for now let's just implement the loop here)
    
    let text = &params.text;
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
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
            if let Some(word) = dict.get_exact(&sub_lower) {
                result.push_str(&word);
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
    result
}