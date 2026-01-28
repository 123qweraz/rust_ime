use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, BufRead};
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NgramModel {
    // context (1 to 3 tokens) -> (next_token -> frequency)
    pub transitions: HashMap<String, HashMap<String, u32>>,
    // 单个词出现的总频率 (Unigram)
    pub unigrams: HashMap<String, u32>,
    pub max_n: usize,
    
    #[serde(skip)]
    pub token_list: Vec<String>,
    #[serde(skip)]
    pub token_set: std::collections::HashSet<String>,
    #[serde(skip)]
    pub max_token_len: usize,
}

impl NgramModel {
    pub fn new() -> Self {
        let mut model = Self {
            transitions: HashMap::new(),
            unigrams: HashMap::new(),
            max_n: 3, // 降级为 3-gram
            token_list: Vec::new(),
            token_set: std::collections::HashSet::new(),
            max_token_len: 0,
        };
        model.load_token_list();
        model
    }

    fn load_token_list(&mut self) {
        let path = Path::new("dicts/basic_tokens.txt");
        if path.exists() {
            if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                self.token_list = reader.lines().filter_map(|l| l.ok()).collect();
                self.max_token_len = 0;
                for t in &self.token_list {
                    let len = t.chars().count();
                    if len > self.max_token_len {
                        self.max_token_len = len;
                    }
                    self.token_set.insert(t.clone());
                }
            }
        }
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut result = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let mut i = 0;

        while i < n {
            let mut found_token = None;
            
            // 贪婪匹配：从最大可能长度开始向下查找 (4 -> 1)
            let max_len = self.max_token_len.min(n - i);
            for len in (1..=max_len).rev() {
                let sub: String = chars[i..i+len].iter().collect();
                if self.token_set.contains(&sub) {
                    found_token = Some(sub);
                    break;
                }
            }

            if let Some(token) = found_token {
                let len = token.chars().count();
                result.push(token);
                i += len;
            } else {
                // 彻底的兜底：如果连单字都没在 token_set 里，只要是汉字就当做 Token
                let c = chars[i];
                if (c >= '\u{4e00}' && c <= '\u{9fa5}') ||
                   (c >= '\u{3400}' && c <= '\u{4dbf}') ||
                   (c >= '\u{20000}' && c <= '\u{2a6df}') {
                    result.push(c.to_string());
                }
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

            // 1. 统计 Unigram (每个词本身出现的次数)
            for token in &tokens {
                *self.unigrams.entry(token.clone()).or_default() += 1;
            }

            // 2. 统计 N-gram 跳转
            if tokens.len() < 2 { continue; }
            for n in 2..=self.max_n {
                if tokens.len() < n { continue; }
                for window in tokens.windows(n) {
                    let context = window[..n-1].join("");
                    let next_token = &window[n-1];
                    let entry = self.transitions.entry(context).or_default();
                    *entry.entry(next_token.clone()).or_default() += 1;
                }
            }
        }
    }

    pub fn update(&mut self, context_chars: &[char], next_token: &str) {
        let token_str = next_token.to_string();
        *self.unigrams.entry(token_str.clone()).or_default() += 1;

        for len in 1..self.max_n {
            if context_chars.len() < len { break; }
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            let entry = self.transitions.entry(context).or_default();
            *entry.entry(token_str.clone()).or_default() += 1;
        }
    }

    pub fn predict(&self, context_chars: &[char], limit: usize) -> Vec<String> {
        // 预测逻辑也应该考虑 Unigram 权重，但先保持 Back-off 逻辑
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                let mut candidates: Vec<(&String, &u32)> = next_map.iter().collect();
                candidates.sort_by(|a, b| b.1.cmp(a.1));
                
                return candidates.into_iter()
                    .take(limit)
                    .map(|(c, _)| c.clone())
                    .collect();
            }
        }
        Vec::new()
    }

    pub fn get_score(&self, context_chars: &[char], next_token_str: &str) -> u32 {
        let mut total_score = *self.unigrams.get(next_token_str).unwrap_or(&0);

        // N-gram 权重放大
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                if let Some(&score) = next_map.get(next_token_str) {
                    // 更加平衡的加权：使用线性 len，乘数降为 10
                    total_score += score * 10 * (len as u32);
                    break; 
                }
            }
        }
        total_score
    }
    
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<() > {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref)?;
        let reader = BufReader::new(file);
        let mut model: Self = serde_json::from_reader(reader)?;
        model.token_set = std::collections::HashSet::new();
        model.load_token_list();
        
        println!("[NgramModel] Successfully loaded from: {:?}", path_ref);
        println!("             Transitions: {}", model.transitions.len());
        println!("             Unigrams:    {}", model.unigrams.len());
        
        Ok(model)
    }
}