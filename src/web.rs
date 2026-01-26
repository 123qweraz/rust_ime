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
use std::sync::{Arc, RwLock, Mutex};
use std::net::SocketAddr;
use std::collections::HashMap;
use serde::Deserialize;
use arboard::Clipboard;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub struct WebServer {
    pub port: u16,
    pub config: Arc<RwLock<Config>>,
    pub tries: Arc<RwLock<HashMap<String, Trie>>>,
    pub clipboard: Arc<Mutex<Option<Clipboard>>>,
}

type WebState = (Arc<RwLock<Config>>, Arc<RwLock<HashMap<String, Trie>>>, Arc<Mutex<Option<Clipboard>>>);

impl WebServer {
    pub fn new(port: u16, config: Arc<RwLock<Config>>, tries: Arc<RwLock<HashMap<String, Trie>>>) -> Self {
        let clipboard = match Clipboard::new() {
            Ok(cb) => Some(cb),
            Err(e) => {
                eprintln!("[Web] Warning: Failed to initialize system clipboard: {}", e);
                None
            }
        };
        Self { 
            port, 
            config, 
            tries,
            clipboard: Arc::new(Mutex::new(clipboard)),
        }
    }

    pub async fn start(self) {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        println!("[Web] 服务器启动在 http://{}", addr);

        let state: WebState = (self.config, self.tries, self.clipboard);

        let app = Router::new()
            .route("/", get(index_handler))
            .route("/api/config", get(get_config))
            .route("/api/config", post(update_config))
            .route("/api/convert", get(convert_handler))
            .route("/api/dicts/list", get(list_dicts))
            .route("/api/dicts/content", get(get_dict_content))
            .route("/api/dicts/save", post(save_dict_content))
            .route("/api/dicts/reload", post(reload_dicts))
            .fallback(static_handler)
            .with_state(state);

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("[Web] 服务器运行错误: {}", e);
                }
            },
            Err(e) => {
                eprintln!("[Web] 无法绑定端口 {}: {}", self.port, e);
            }
        }
    }
}

async fn index_handler() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => {
             match std::str::from_utf8(&content.data) {
                Ok(html) => Html(html.to_string()).into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
             }
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
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

async fn get_config(State((config, _, _)): State<WebState>) -> Json<Config> {

    Json(config.read().unwrap().clone())

}



async fn update_config(

    State((config, _, _)): State<WebState>,

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



    copy: Option<bool>,



    list: Option<usize>,



    page: Option<usize>,



    all: Option<bool>,



}







async fn convert_handler(







    State((config, tries, clipboard)): State<WebState>,







    Query(params): Query<ConvertParams>,







) -> String {







    let c = config.read().unwrap();







    let t = tries.read().unwrap();







    let active_profile = &c.input.default_profile;







    let dict = t.get(active_profile);















    let mut final_result = String::new();







    







    // 将输入按空格拆分（处理多个参数或手动分词）







    let words: Vec<&str> = params.text.split_whitespace().collect();















    for word in words {







        // 1. 处理单个单词的逃逸字符 /







        if word.starts_with('/') {







            final_result.push_str(&word[1..]);







            continue;







        }















        // 2. 如果没有字典，原样输出







        let dict = match dict {







            Some(d) => d,







            None => {







                final_result.push_str(word);







                continue;







            }







        };















        // 3. 解析数字选择 (例如 ni1)







        let mut clean_word = word.to_string();







        let mut selected_idx = None;







        let mut num_str = String::new();







        while let Some(last_char) = clean_word.chars().last() {







            if last_char.is_ascii_digit() {







                num_str.insert(0, clean_word.pop().unwrap());







            } else {







                break;







            }







        }







        if !num_str.is_empty() {







            selected_idx = num_str.parse::<usize>().ok();







        }















        // 4. 判断模式







        let is_query_mode = params.all.unwrap_or(false) || params.list.is_some() || selected_idx.is_some();















        if is_query_mode {







            // --- 单词模式 ---







            let mut pinyin_search = clean_word;







                        let mut _filter_string = String::new();







                        if let Some((idx, _)) = pinyin_search.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {







                            _filter_string = pinyin_search[idx..].to_lowercase();







                            pinyin_search = pinyin_search[..idx].to_string();







                        }















            let raw_candidates = dict.search_bfs(&pinyin_search.to_lowercase(), 100);















            if let Some(idx) = selected_idx {







                if idx > 0 && idx <= raw_candidates.len() {







                    final_result.push_str(&raw_candidates[idx - 1]);







                } else {







                    final_result.push_str(&pinyin_search); // 索引无效回退







                }







            } else if params.all.unwrap_or(false) {







                final_result.push_str(&raw_candidates.join(" "));







            } else if let Some(limit) = params.list {







                let page = params.page.unwrap_or(1).max(1);







                let start = (page - 1) * limit;







                if start < raw_candidates.len() {







                    let end = (start + limit).min(raw_candidates.len());







                    final_result.push_str(&raw_candidates[start..end].join(" "));







                }







            }







            else {







                final_result.push_str(raw_candidates.first().unwrap_or(&pinyin_search));







            }







        } else {







            // --- 全句转换模式 ---







            let chars: Vec<char> = word.chars().collect();







            let mut i = 0;







            while i < chars.len() {







                if !chars[i].is_ascii_alphabetic() {







                    final_result.push(chars[i]);







                    i += 1;







                    continue;







                }















                let mut found = false;







                for len in (1..=(chars.len() - i).min(15)).rev() {







                    let sub: String = chars[i..i+len].iter().collect();







                    let sub_lower = sub.to_lowercase();







                    if let Some(word_match) = dict.get_exact(&sub_lower) {







                        final_result.push_str(&word_match);







                        i += len;







                        found = true;







                        break;







                    }







                }







                if !found {







                    final_result.push(chars[i]);







                    i += 1;







                }







            }







        }







    }















    // Handle Server-side Copy







    if params.copy.unwrap_or(false) {







        let text_to_copy = final_result.clone();







        let clipboard_state = Arc::clone(&clipboard);







        std::thread::spawn(move || {







            if let Ok(mut guard) = clipboard_state.lock() {







                if let Some(cb) = guard.as_mut() {







                    let _ = cb.set_text(text_to_copy);







                }







            }







        });







    }















        final_result















    }















    















    // --- 词典编辑器 API ---















    















    #[derive(serde::Serialize)]















    struct DictFile {















        name: String,















        path: String,















    }















    















    async fn list_dicts() -> Json<Vec<DictFile>> {















        let mut list = Vec::new();















        let root = "dicts";















        for entry in walkdir::WalkDir::new(root) {















            if let Ok(entry) = entry {















                if entry.path().is_file() && entry.path().extension().map_or(false, |ext| ext == "json") {















                    let path_str = entry.path().to_string_lossy().to_string();















                    let name = entry.path().strip_prefix(root).unwrap_or(entry.path()).to_string_lossy().to_string();















                    list.push(DictFile { name, path: path_str });















                }















            }















        }















        list.sort_by(|a, b| a.name.cmp(&b.name));















        Json(list)















    }















    















    async fn get_dict_content(Query(params): Query<HashMap<String, String>>) -> Result<Json<serde_json::Value>, StatusCode> {















        let path = params.get("path").ok_or(StatusCode::BAD_REQUEST)?;















        if !path.starts_with("dicts/") || path.contains("..") {















            return Err(StatusCode::FORBIDDEN);















        }















    















        let file = std::fs::File::open(path).map_err(|_| StatusCode::NOT_FOUND)?;















        let reader = std::io::BufReader::new(file);















        let content: serde_json::Value = serde_json::from_reader(reader).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;















        Ok(Json(content))















    }















    















    #[derive(serde::Deserialize)]















    struct SaveDictParams {















        path: String,















        content: serde_json::Value,















    }















    















    async fn save_dict_content(Json(params): Json<SaveDictParams>) -> StatusCode {















        if !params.path.starts_with("dicts/") || params.path.contains("..") {















            return StatusCode::FORBIDDEN;















        }















    















        let file = match std::fs::File::create(&params.path) {















            Ok(f) => f,















            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,















        };















        















        if let Err(_) = serde_json::to_writer_pretty(file, &params.content) {















            return StatusCode::INTERNAL_SERVER_ERROR;















        }















    















        println!("[Web] 词典文件已保存: {}", params.path);















        StatusCode::OK















    }















    















    async fn reload_dicts(State((config, tries, _)): State<WebState>) -> StatusCode {















        use crate::load_dict_for_profile;















        















        let c = config.read().unwrap();















        let mut new_tries = HashMap::new();















        















        println!("[Web] 正在重新加载所有词典...");















        for profile in &c.files.profiles {















            let trie = load_dict_for_profile(&profile.dicts);















            new_tries.insert(profile.name.clone(), trie);















        }















    















        {















            let mut t = tries.write().unwrap();















            *t = new_tries;















        }















        















        println!("[Web] 词典重载完成。");















        StatusCode::OK















    }















    






