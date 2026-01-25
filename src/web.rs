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



    let text = &params.text;



    



    // 1. 处理逃逸字符 /



    if text.starts_with('/') {



        return text[1..].to_string();



    }







    let c = config.read().unwrap();



    let t = tries.read().unwrap();



    let active_profile = &c.input.default_profile;



    let dict = match t.get(active_profile) {



        Some(d) => d,



        None => return text.clone(),



    };







    // 2. 解析数字选择 (例如 ni1, nihao10)



    let mut clean_text = text.clone();



    let mut selected_idx = None;



    



    // 从末尾寻找数字



    let mut num_str = String::new();



    while let Some(last_char) = clean_text.chars().last() {



        if last_char.is_ascii_digit() {



            num_str.insert(0, clean_text.pop().unwrap());



        } else {



            break;



        }



    }



    if !num_str.is_empty() {



        selected_idx = num_str.parse::<usize>().ok();



    }







    // 3. 判断是进行“单词查词”还是“全句转换”



    // 如果有 -l, -a, 或者有数字索引，或者只有单个词的感觉，进入单词模式



    let is_query_mode = params.all.unwrap_or(false) || params.list.is_some() || selected_idx.is_some();







    let result = if is_query_mode {



        // --- 单词模式 (Lookup Logic) ---



        let mut pinyin_search = clean_text.clone();



        let mut filter_string = String::new();







        // 处理大写字母辅助过滤 (niN -> ni, n)



        if let Some((idx, _)) = clean_text.char_indices().skip(1).find(|(_, c)| c.is_ascii_uppercase()) {



            pinyin_search = clean_text[..idx].to_string();



            filter_string = clean_text[idx..].to_lowercase();



        }







        // 获取候选词 (简化版搜索，不带模糊音以保证准确性)



        let raw_candidates = dict.search_bfs(&pinyin_search.to_lowercase(), 100);







        // 应用英文过滤



        if !filter_string.is_empty() {



            // 需要加载 word_en_map，但在 WebState 里没存，我们暂时从磁盘读取或先跳过高级过滤



            // 为了性能和实现，这里可以先根据 profile 字典查找



        }







        if let Some(idx) = selected_idx {



            // ni1 对应索引 0



            if idx > 0 && idx <= raw_candidates.len() {



                raw_candidates[idx - 1].clone()



            } else {



                clean_text // 没找到索引，返回原文



            }



        } else if params.all.unwrap_or(false) {



            raw_candidates.join(" ")



        } else if let Some(limit) = params.list {



            let page = params.page.unwrap_or(1).max(1);



            let start = (page - 1) * limit;



            if start < raw_candidates.len() {



                let end = (start + limit).min(raw_candidates.len());



                raw_candidates[start..end].join(" ")



            } else {



                String::new()



            }



        } else {



            raw_candidates.first().cloned().unwrap_or(clean_text)



        }



    } else {



        // --- 全句模式 (Convert Logic) ---



        let mut converted = String::new();



        let chars: Vec<char> = text.chars().collect();



        let mut i = 0;







        while i < chars.len() {



            if !chars[i].is_ascii_alphabetic() {



                converted.push(chars[i]);



                i += 1;



                continue;



            }







            let mut found = false;



            for len in (1..=(chars.len() - i).min(15)).rev() {



                let sub: String = chars[i..i+len].iter().collect();



                let sub_lower = sub.to_lowercase();



                if let Some(word) = dict.get_exact(&sub_lower) {



                    converted.push_str(&word);



                    i += len;



                    found = true;



                    break;



                }



            }







            if !found {



                converted.push(chars[i]);



                i += 1;



            }



        }



        converted



    };







    // Handle Server-side Copy



    if params.copy.unwrap_or(false) {



        let text_to_copy = result.clone();



        let clipboard_state = Arc::clone(&clipboard);



        std::thread::spawn(move || {



            if let Ok(mut guard) = clipboard_state.lock() {



                if let Some(cb) = guard.as_mut() {



                    let _ = cb.set_text(text_to_copy);



                }



            }



        });



    }







    result



}






