use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, BufRead};
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NgramModel {
    // context (1 to 3 tokens) -> (next_token -> frequency)
    pub transitions: HashMap<String, HashMap<String, u32>>,
    pub max_n: usize,
    
    #[serde(skip)]
    pub token_list: Vec<String>,
}

impl NgramModel {
    pub fn new() -> Self {
        let mut model = Self {
            transitions: HashMap::new(),
            max_n: 4,
            token_list: Vec::new(),
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
                // Ensure they are sorted by length descending for greedy match
                self.token_list.sort_by(|a, b| b.len().cmp(&a.len()));
            }
        }
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut current_offset = 0;
        let chars: Vec<char> = text.chars().collect();
        let text_len = chars.len();

        while current_offset < text_len {
            let remaining = &chars[current_offset..].iter().collect::<String>();
            let mut found = false;

            for token in &self.token_list {
                if remaining.starts_with(token) {
                    result.push(token.clone());
                    current_offset += token.chars().count();
                    found = true;
                    break;
                }
            }

            if !found {
                // Fallback to single character
                let c = chars[current_offset];
                if (c >= '\u{4e00}' && c <= '\u{9fa5}') ||
                   (c >= '\u{3400}' && c <= '\u{4dbf}') ||
                   (c >= '\u{20000}' && c <= '\u{2a6df}') {
                    result.push(c.to_string());
                }
                current_offset += 1;
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
            if tokens.len() < 2 {
                continue;
            }

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

    pub fn update(&mut self, context_chars: &[char], next_char: char) {
        // Simple character update for live learning, as it's harder to tokenize live stream reliably
        // but we treat next_char as a single-char string
        let next_token = next_char.to_string();
        for len in 1..self.max_n {
            if context_chars.len() < len { break; }
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            
            let entry = self.transitions.entry(context).or_default();
            *entry.entry(next_token.clone()).or_default() += 1;
        }
    }

    pub fn predict(&self, context_chars: &[char], limit: usize) -> Vec<String> {
        // Try different context lengths from the context buffer
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

    pub fn get_score(&self, context_chars: &[char], next_char: char) -> u32 {
        let next_token = next_char.to_string();
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                if let Some(&score) = next_map.get(&next_token) {
                    return score * (len as u32);
                }
            }
        }
        0
    }
    
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut model: Self = serde_json::from_reader(reader)?;
        model.load_token_list();
        Ok(model)
    }
}
