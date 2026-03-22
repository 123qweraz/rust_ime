use axum::{
    routing::{get, post},
    extract::{State, Json},
    response::{IntoResponse, Html},
    http::{StatusCode, Uri},
    Router,
};
use serde::{Serialize};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU16, Ordering};
use std::collections::HashMap;
use rust_ime_core::Config;
use rust_ime_engine::trie::Trie;
use rust_embed::RustEmbed;
use crate::tray::TrayEvent;

#[derive(RustEmbed)]
#[folder = "../../static/"]
struct Assets;

// Web server implementation for IME configuration
pub struct WebServer {
    pub port: u16,
    pub actual_port: Arc<AtomicU16>,
    pub config: Arc<RwLock<Config>>,
    pub tries: Arc<RwLock<HashMap<String, Trie>>>,
    pub tray_tx: std::sync::mpsc::Sender<TrayEvent>,
}

type WebState = (
    Arc<RwLock<Config>>, 
    Arc<RwLock<HashMap<String, Trie>>>, 
    std::sync::mpsc::Sender<TrayEvent>
);

impl WebServer {
    pub fn new(
        port: u16, 
        actual_port: Arc<AtomicU16>,
        config: Arc<RwLock<Config>>, 
        tries: Arc<RwLock<HashMap<String, Trie>>>,
        tray_tx: std::sync::mpsc::Sender<TrayEvent>
    ) -> Self {
        Self { port, actual_port, config, tries, tray_tx }
    }

    pub async fn start(self) {
        let state: WebState = (self.config, self.tries, self.tray_tx);
        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config).post(update_config))
            .route("/api/config/reset", post(reset_config))
            .route("/api/fonts", get(list_fonts))
            .route("/api/dicts", get(list_dicts))
            .route("/api/dicts/compile", post(compile_dicts_handler))
            .route("/api/dicts/reload", post(reload_dicts))
            .route("/api/dicts/toggle", post(toggle_dict))
            .route("/api/dictionary/chars", get(get_chars_dict))
            .route("/api/dict/search", get(search_dict))
            .route("/api/dict/update", post(update_dict_entry))
            .route("/api/dict/add", post(add_dict_entry))
            .route("/api/dict/clear_user", post(clear_user_dict))
            .route("/static/*file", get(static_handler))
            .route("/dicts/*file", get(dicts_handler))
            .fallback(index_handler)
            .with_state(state);

        let mut current_port = self.port;
        loop {
            let addr = format!("127.0.0.1:{}", current_port);
            match tokio::net::TcpListener::bind(&addr).await {
                Ok(listener) => {
                    self.actual_port.store(current_port, Ordering::SeqCst);
                    println!("[Web] 服务器启动在 http://{}", addr);
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("[Web] Server error: {}", e);
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    eprintln!("[Web] 端口 {} 已被占用，正在尝试 {}...", current_port, current_port + 1);
                    current_port += 1;
                    if current_port > self.port + 100 {
                        eprintln!("[Web] 已尝试 100 个端口均无法启动，退出。");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[Web] Failed to bind to {}: {}", addr, e);
                    break;
                }
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

async fn dicts_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches("/dicts/").trim_start_matches("/");
    
    // 寻找词典目录的逻辑：优先当前目录，否则相对于可执行文件查找
    let mut base_path = std::path::PathBuf::from("dicts");
    if !base_path.exists() {
        if let Ok(mut exe_path) = std::env::current_exe() {
            exe_path.pop();
            let mut p = exe_path.clone();
            // 向上查找最多三层，匹配 IDE 开发环境
            for _ in 0..3 {
                let check_p = p.join("dicts");
                if check_p.exists() { base_path = check_p; break; }
                p.pop();
            }
        }
    }

    let full_path = base_path.join(path);
    
    if let Ok(content) = std::fs::read(&full_path) {
        let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
        ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], content).into_response()
    } else {
        eprintln!("[Web] Dictionary file not found: {:?}", full_path);
        (StatusCode::NOT_FOUND, "Dictionary Not Found").into_response()
    }
}

async fn get_config(State((_, _, _)): State<WebState>) -> impl IntoResponse {
    // 实时读取最新的分段配置
    Json(crate::Config::load()).into_response()
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
        
        // 处理自启逻辑变化
        if w.input.autostart != new_config.input.autostart {
            if new_config.input.autostart {
                let _ = crate::platform::setup_autostart();
            } else {
                let _ = crate::platform::remove_autostart();
            }
        }

        *w = new_config.clone();
    }
    // 使用新重构的 save 方法保存到 configs/ 目录
    if let Err(_e) = new_config.save() { return StatusCode::INTERNAL_SERVER_ERROR; }
    let _ = tray_tx.send(TrayEvent::ReloadConfig);
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
    if let Err(_e) = default_conf.save() { return StatusCode::INTERNAL_SERVER_ERROR; }
    let _ = tray_tx.send(TrayEvent::ReloadConfig);
    StatusCode::OK
}

#[derive(Serialize)]
struct DictFile {
    name: String,
    path: String,
    group: String,
    size: u64,
    entry_count: u64,
    enabled: bool,
}

async fn list_dicts() -> Json<Vec<DictFile>> {
    let mut list = Vec::new();
    let root = "dicts";
    let walker = walkdir::WalkDir::new(root).into_iter();
    
    for entry in walker.filter_map(|e: Result<walkdir::DirEntry, walkdir::Error>| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if filename.ends_with(".json") || filename.ends_with(".json.disabled") {
                // 计算分组名：取 dicts/ 下的一级目录名
                let relative = path.strip_prefix(root).unwrap_or(path);
                let group = relative.components().next()
                    .map(|c: std::path::Component| c.as_os_str().to_string_lossy().to_string())
                    .unwrap_or_else(|| "other".to_string());
                
                let mut dict = process_dict_entry(path.to_path_buf());
                dict.group = group;
                list.push(dict);
            }
        }
    }
    Json(list)
}

fn process_dict_entry(path: std::path::PathBuf) -> DictFile {
    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let enabled = !filename.contains(".disabled");
    let metadata = path.metadata();
    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
    
    let mut entry_count = 0;
    if let Ok(f) = std::fs::File::open(&path) {
        if let Ok(json) = serde_json::from_reader::<_, serde_json::Value>(std::io::BufReader::new(f)) {
            if let Some(obj) = json.as_object() {
                for val in obj.values() {
                    if let Some(arr) = val.as_array() { entry_count += arr.len() as u64; } else { entry_count += 1; }
                }
            } else if let Some(arr) = json.as_array() { entry_count = arr.len() as u64; }
        }
    }

    DictFile {
        name: filename,
        path: path.to_string_lossy().to_string(),
        group: String::new(),
        size,
        entry_count,
        enabled,
    }
}

#[derive(serde::Deserialize)]
struct ToggleRequest {
    path: String,
}

async fn toggle_dict(Json(req): Json<ToggleRequest>) -> StatusCode {
    let path = std::path::Path::new(&req.path);
    if !path.exists() { return StatusCode::NOT_FOUND; }

    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let new_path = if filename.ends_with(".json") {
        path.with_file_name(format!("{}.disabled", filename))
    } else if filename.ends_with(".json.disabled") {
        path.with_file_name(filename.replace(".json.disabled", ".json"))
    } else {
        return StatusCode::BAD_REQUEST;
    };

    if std::fs::rename(path, new_path).is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn compile_dicts_handler(State((_, _, tray_tx)): State<WebState>) -> StatusCode {
    let _ = tray_tx.send(TrayEvent::ShowNotification("正在编译词库...".into()));
    match rust_ime_engine::compiler::check_and_compile_all() {
        Ok(_) => {
            let _ = tray_tx.send(TrayEvent::ShowNotification("词库编译完成".into()));
            // 编译完成后自动重载
            let _ = tray_tx.send(TrayEvent::ReloadConfig);
            StatusCode::OK
        },
        Err(e) => {
            eprintln!("[Web] 词库编译失败: {}", e);
            let _ = tray_tx.send(TrayEvent::ShowNotification(format!("编译失败: {}", e)));
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn reload_dicts(State((_, _, tray_tx)): State<WebState>) -> StatusCode {
    let _ = tray_tx.send(TrayEvent::ReloadConfig);
    StatusCode::OK
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Serialize)]
struct SearchResult {
    pinyin: String,
    word: String,
    hint: String,
    file: String,
}

async fn search_dict(axum::extract::Query(query): axum::extract::Query<SearchQuery>) -> Json<Vec<SearchResult>> {
    let mut results = Vec::new();
    let q = query.q.to_lowercase();
    
    // 遍历 dicts 目录下所有有效的 json
    let root = "dicts";
    let entries = walkdir::WalkDir::new(root);
    for entry in entries.into_iter().filter_map(|e: Result<walkdir::DirEntry, walkdir::Error>| e.ok()) {
        if entry.path().extension().is_some_and(|ext: &std::ffi::OsStr| ext == "json") {
            let path_str = entry.path().to_string_lossy().to_string();
            if let Ok(f) = std::fs::File::open(entry.path()) {
                if let Ok(json) = serde_json::from_reader::<_, serde_json::Value>(std::io::BufReader::new(f)) {
                    if let Some(obj) = json.as_object() {
                        for (pinyin, val) in obj {
                            if let Some(arr) = val.as_array() {
                                for v in arr {
                                    let word = v.get("char").and_then(|c| c.as_str()).unwrap_or("");
                                    let hint = v.get("en").and_then(|e| e.as_str()).unwrap_or("");
                                    if pinyin.to_lowercase().contains(&q) || word.contains(&q) {
                                        results.push(SearchResult {
                                            pinyin: pinyin.clone(),
                                            word: word.to_string(),
                                            hint: hint.to_string(),
                                            file: path_str.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if results.len() > 100 { break; } // 限制结果数量
    }
    Json(results)
}

#[derive(serde::Deserialize)]
struct UpdateEntryRequest {
    pinyin: String,
    word: String,
    new_hint: String,
    file: String,
}

async fn update_dict_entry(Json(req): Json<UpdateEntryRequest>) -> StatusCode {
    let path = std::path::Path::new(&req.file);
    if !path.exists() { return StatusCode::NOT_FOUND; }

    let mut data: serde_json::Value = match std::fs::File::open(path) {
        Ok(f) => serde_json::from_reader(std::io::BufReader::new(f)).unwrap_or(serde_json::Value::Null),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    let mut success = false;
    if let Some(obj) = data.as_object_mut() {
        if let Some(entries) = obj.get_mut(&req.pinyin).and_then(|v| v.as_array_mut()) {
            for entry in entries {
                if entry.get("char").and_then(|c| c.as_str()) == Some(&req.word) {
                    entry["en"] = serde_json::Value::String(req.new_hint.clone());
                    success = true;
                    break;
                }
            }
        }
    }

    if success {
        if let Ok(f) = std::fs::File::create(path) {
            if serde_json::to_writer_pretty(f, &data).is_ok() {
                return StatusCode::OK;
            }
        }
    }

    StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(serde::Deserialize)]
struct AddEntryRequest {
    pinyin: String,
    word: String,
    hint: String,
    file: String,
}

async fn add_dict_entry(Json(req): Json<AddEntryRequest>) -> StatusCode {
    let path = std::path::Path::new(&req.file);
    if !path.exists() { return StatusCode::NOT_FOUND; }

    let mut data: serde_json::Value = match std::fs::File::open(path) {
        Ok(f) => serde_json::from_reader(std::io::BufReader::new(f)).unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    if let Some(obj) = data.as_object_mut() {
        let entries = obj.entry(req.pinyin).or_insert(serde_json::Value::Array(Vec::new()));
        if let Some(arr) = entries.as_array_mut() {
            // 检查是否已存在
            for item in arr.iter() {
                if item.get("char").and_then(|c| c.as_str()) == Some(&req.word) {
                    return StatusCode::CONFLICT;
                }
            }
            let mut new_entry = serde_json::Map::new();
            new_entry.insert("char".to_string(), serde_json::Value::String(req.word));
            new_entry.insert("en".to_string(), serde_json::Value::String(req.hint));
            arr.push(serde_json::Value::Object(new_entry));
        }
    }

    if let Ok(f) = std::fs::File::create(path) {
        if serde_json::to_writer_pretty(f, &data).is_ok() {
            return StatusCode::OK;
        }
    }

    StatusCode::INTERNAL_SERVER_ERROR
}

async fn clear_user_dict(State((_, _, tray_tx)): State<WebState>) -> StatusCode {
    let files = ["data/learned_words.json", "data/usage_history.json", "data/user_dict.json"];
    for f in files {
        let path = std::path::Path::new(f);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
    // 通知主线程清空内存中的用户词典
    let _ = tray_tx.send(TrayEvent::ClearUserDict);
    StatusCode::OK
}

async fn list_fonts() -> Json<Vec<crate::platform::fonts::FontInfo>> {
    Json(crate::platform::fonts::list_system_fonts())
}

#[derive(Serialize)]
struct CharEntryView {
    pinyin: String,
    #[serde(rename = "char")]
    character: String,
    en_meaning: String,
    en_aux: String,
    stroke_aux: String,
    group: u32,
}

#[derive(serde::Deserialize)]
struct DictViewQuery {
    file: Option<String>,
}

async fn get_chars_dict(axum::extract::Query(query): axum::extract::Query<DictViewQuery>) -> impl IntoResponse {
    let filename = query.file.unwrap_or_else(|| "dicts/chinese/chars/chars.json".to_string());
    let path = std::path::Path::new(&filename);
    let mut results = Vec::new();
    
    if let Ok(f) = std::fs::File::open(path) {
        if let Ok(json) = serde_json::from_reader::<_, serde_json::Value>(std::io::BufReader::new(f)) {
            if let Some(obj) = json.as_object() {
                let mut pinyin_sorted: Vec<_> = obj.keys().collect();
                pinyin_sorted.sort();
                
                let mut group_toggle = 0;
                let mut last_pinyin = String::new();
                
                for pinyin in pinyin_sorted {
                    if pinyin != &last_pinyin && !last_pinyin.is_empty() {
                        group_toggle = 1 - group_toggle;
                    }
                    last_pinyin = pinyin.clone();
                    
                    if let Some(entries) = obj.get(pinyin).and_then(|v| v.as_array()) {
                        for entry in entries {
                            let character = entry.get("char").and_then(|v| v.as_str()).unwrap_or("");
                            let en_meaning = entry.get("en").and_then(|v| v.as_str()).unwrap_or("");
                            let stroke_code = entry.get("stroke_aux").and_then(|v| v.as_str()).unwrap_or("");
                            
                            // 英文辅助码：拼音 + 英文前3位 (如果en存在)
                            let en_aux = if !en_meaning.is_empty() {
                                format!("{}{}", pinyin, en_meaning.chars().take(3).collect::<String>())
                            } else {
                                pinyin.clone()
                            };
                            
                            // 笔画辅助码：拼音 + 笔画码 (如果笔画存在)
                            let stroke_aux = if !stroke_code.is_empty() {
                                format!("{}{}", pinyin, stroke_code)
                            } else {
                                pinyin.clone()
                            };
                            
                            results.push(CharEntryView {
                                pinyin: pinyin.clone(),
                                character: character.to_string(),
                                en_meaning: if en_meaning.is_empty() { "-".to_string() } else { en_meaning.to_string() },
                                en_aux,
                                stroke_aux: if stroke_code.is_empty() { "-".to_string() } else { stroke_aux },
                                group: group_toggle,
                            });
                        }
                    }
                }
            }
        }
    }
    Json(results).into_response()
}
