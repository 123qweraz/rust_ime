use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufReader, BufRead};
use std::path::Path;
use serde::{Serialize, Deserialize};
use memmap2::Mmap;
use fst::{Map};
use std::sync::Arc;

#[derive(Clone)]
pub struct MmapData(Arc<Mmap>);
impl AsRef<[u8]> for MmapData {
    fn as_ref(&self) -> &[u8] { self.0.as_ref() }
}

#[derive(Clone)]
pub struct NgramModel {
    // 静态层 (Mmap)
    static_index: Option<Map<MmapData>>,
    static_unigrams: Option<Map<MmapData>>,
    static_data: Option<MmapData>,

    // 动态层 (Memory) - 仅用于用户实时学习
    pub user_transitions: HashMap<String, HashMap<String, u32>>,
    pub user_unigrams: HashMap<String, u32>,
    
    pub max_n: usize,
    pub token_set: HashSet<String>,
    pub max_token_len: usize,
}

impl NgramModel {
    pub fn new() -> Self {
        let mut model = Self {
            static_index: None,
            static_unigrams: None,
            static_data: None,
            user_transitions: HashMap::new(),
            user_unigrams: HashMap::new(),
            max_n: 3,
            token_set: HashSet::new(),
            max_token_len: 0,
        };
        model.load_token_list();
        model.load_static_model();
        model
    }

    fn load_static_model(&mut self) {
        let idx_path = "data/ngram.index";
        let data_path = "data/ngram.data";
        let uni_path = "data/ngram.unigram";

        if Path::new(idx_path).exists() && Path::new(data_path).exists() {
            if let (Ok(f_idx), Ok(f_data), Ok(f_uni)) = (File::open(idx_path), File::open(data_path), File::open(uni_path)) {
                if let (Ok(m_idx), Ok(m_data), Ok(m_uni)) = (unsafe { Mmap::map(&f_idx) }, unsafe { Mmap::map(&f_data) }, unsafe { Mmap::map(&f_uni) }) {
                    self.static_index = Map::new(MmapData(Arc::new(m_idx))).ok();
                    self.static_unigrams = Map::new(MmapData(Arc::new(m_uni))).ok();
                    self.static_data = Some(MmapData(Arc::new(m_data)));
                }
            }
        }
    }

    fn load_token_list(&mut self) {
        let path = Path::new("dicts/chinese/basic_tokens.txt");
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let len = line.chars().count();
                if len > self.max_token_len { self.max_token_len = len; }
                self.token_set.insert(line);
            }
        }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut result = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let mut i = 0;
        while i < n {
            let mut found_token = None;
            let max_len = self.max_token_len.min(n - i);
            for len in (1..=max_len).rev() {
                let sub: String = chars[i..i+len].iter().collect();
                if self.token_set.contains(&sub) { found_token = Some(sub); break; }
            }
            if let Some(token) = found_token {
                let len = token.chars().count();
                result.push(token); i += len;
            } else {
                let c = chars[i];
                if (c >= '\u{4e00}' && c <= '\u{9fa5}') || (c >= '\u{3400}' && c <= '\u{4dbf}') || (c >= '\u{20000}' && c <= '\u{2a6df}') { result.push(c.to_string()); }
                i += 1;
            }
        }
        result
    }

    pub fn train(&mut self, text: &str) {
        let sections = text.split(|c: char| {
            c == '\n' || c == '\r' || c == '。' || c == '，' || c == '！' || c == '？' || c == '；' || c == '：' || c == '“' || c == '”' || c == '（' || c == '）' || c == '、'
        });
        for section in sections {
            let tokens = self.tokenize(section);
            if tokens.is_empty() { continue; }
            let mut char_level_tokens = Vec::new();
            for token in &tokens {
                *self.user_unigrams.entry(token.clone()).or_default() += 1;
                let chars: Vec<char> = token.chars().collect();
                for &c in &chars {
                    let c_str = c.to_string();
                    if chars.len() > 1 { *self.user_unigrams.entry(c_str.clone()).or_default() += 1; }
                    char_level_tokens.push(c_str);
                }
            }
            if tokens.len() >= 2 {
                for n in 2..=self.max_n {
                    if tokens.len() < n { continue; }
                    for window in tokens.windows(n) {
                        let context = window[..n-1].join("");
                        let next_token = &window[n-1];
                        let entry = self.user_transitions.entry(context).or_default();
                        *entry.entry(next_token.clone()).or_default() += 1;
                    }
                }
            }
            if char_level_tokens.len() >= 2 {
                for n in 2..=self.max_n {
                    if char_level_tokens.len() < n { continue; }
                    for window in char_level_tokens.windows(n) {
                        let context = window[..n-1].join("");
                        let next_token = &window[n-1];
                        let entry = self.user_transitions.entry(context).or_default();
                        *entry.entry(next_token.clone()).or_default() += 1;
                    }
                }
            }
        }
    }

    pub fn update(&mut self, context_chars: &[char], next_token: &str) {
        let token_str = next_token.to_string();
        *self.user_unigrams.entry(token_str.clone()).or_default() += 1;
        for len in 1..self.max_n {
            if context_chars.len() < len { break; }
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            let entry = self.user_transitions.entry(context).or_default();
            *entry.entry(token_str.clone()).or_default() += 1;
        }
    }

    pub fn get_score(&self, context_chars: &[char], next_token_str: &str) -> u32 {
        let mut total_score = 0u32;
        if let Some(ref static_uni) = self.static_unigrams { total_score += static_uni.get(next_token_str).unwrap_or(0) as u32; }
        total_score += self.user_unigrams.get(next_token_str).cloned().unwrap_or(0);
        let target_bytes = next_token_str.as_bytes();
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            let mut found_context = false;
            if let (Some(ref idx), Some(ref data)) = (&self.static_index, &self.static_data) {
                if let Some(offset) = idx.get(&context) {
                    let score = self.scan_score_in_block(offset as usize, data.as_ref(), target_bytes);
                    if score > 0 { total_score += score * 10 * (len as u32); found_context = true; }
                }
            }
            if let Some(next_map) = self.user_transitions.get(&context) {
                if let Some(&score) = next_map.get(next_token_str) { total_score += score * 100 * (len as u32); found_context = true; }
            }
            if found_context { break; }
        }
        total_score
    }

    fn scan_score_in_block(&self, offset: usize, data: &[u8], target_bytes: &[u8]) -> u32 {
        let mut cursor = offset;
        let count = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
        cursor += 4;
        for _ in 0..count {
            let len = u16::from_le_bytes(data[cursor..cursor+2].try_into().unwrap()) as usize;
            cursor += 2;
            let word_bytes = &data[cursor..cursor+len];
            if word_bytes == target_bytes {
                cursor += len;
                return u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
            }
            cursor += len + 4;
        }
        0
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let writer = io::BufWriter::new(file);
        let user_data = UserAdapter {
            transitions: self.user_transitions.clone(),
            unigrams: self.user_unigrams.clone(),
        };
        serde_json::to_writer(writer, &user_data)?;
        Ok(())
    }

    pub fn load_user_adapter<P: AsRef<Path>>(&mut self, path: P) {
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            if let Ok(adapter) = serde_json::from_reader::<_, UserAdapter>(reader) {
                self.user_transitions = adapter.transitions;
                self.user_unigrams = adapter.unigrams;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct UserAdapter {
    transitions: HashMap<String, HashMap<String, u32>>,
    unigrams: HashMap<String, u32>,
}
